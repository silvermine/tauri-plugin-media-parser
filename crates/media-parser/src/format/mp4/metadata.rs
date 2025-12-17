//! # MP4 Metadata Extraction
//!
//! Extracts metadata from MP4 files:
//! - Duration and timescale from `mvhd` box
//! - Tags (title, artist, album, etc.) from `ilst` box
//!
//! Navigation traverses byte slices directly without intermediate deserialization.

use super::atoms::{Mp4Box, Mp4Nav, find_and_read_moov_box, fourcc_to_key, iter_boxes, tag_name};
use crate::Result;
use crate::errors::MediaParserError;
use crate::helpers::{
   decode_utf8, decode_utf16_with_bom, read_u32_be, read_u64_be, trim_null_and_whitespace,
};
use crate::stream::StreamReader;
use crate::types::{self, Metadata};

/// mvhd version 0 layout (32-bit time fields)
const MVHD_V0_TIMESCALE_OFFSET: usize = 12;
const MVHD_V0_DURATION_OFFSET: usize = 16;
const MVHD_V0_MIN_SIZE: usize = 20;

/// mvhd version 1 layout (64-bit time fields)
const MVHD_V1_TIMESCALE_OFFSET: usize = 20;
const MVHD_V1_DURATION_OFFSET: usize = 24;
const MVHD_V1_MIN_SIZE: usize = 32;

/// Reads metadata from an MP4 file.
///
/// Extracts:
/// - Duration and timescale from the `mvhd` box
/// - Metadata tags (title, artist, etc.) from the `ilst` box
///
/// # Errors
///
/// Returns an error if:
/// - The `moov` box cannot be found
/// - The `mvhd` box is missing or corrupted
pub async fn read_metadata(reader: &dyn StreamReader) -> Result<Metadata> {
   let moov_data = find_and_read_moov_box(reader).await?;

   let moov_payload = if moov_data.len() >= 8 && &moov_data[4..8] == b"moov" {
      &moov_data[8..]
   } else {
      &moov_data
   };

   let (timescale, duration) = extract_mvhd_info(moov_payload)?;
   let values = extract_metadata_tags(moov_payload);

   Ok(Metadata {
      format: "MP4/M4A/MOV".to_string(),
      values,
      timescale,
      duration,
   })
}

/// Extracts timescale and duration from the mvhd box.
fn extract_mvhd_info(moov_payload: &[u8]) -> Result<(u32, u64)> {
   let mvhd_path = [Mp4Box::Mvhd.bytes()];
   let mvhd = moov_payload
      .nav(&mvhd_path)
      .ok_or_else(|| MediaParserError::InvalidFormat("mvhd box not found".to_string()))?;

   if mvhd.is_empty() {
      return Err(MediaParserError::CorruptedData(0));
   }

   let version = mvhd[0];

   match version {
      0 => {
         if mvhd.len() < MVHD_V0_MIN_SIZE {
            return Err(MediaParserError::CorruptedData(mvhd.len() as u64));
         }
         let timescale = read_u32_be(mvhd, MVHD_V0_TIMESCALE_OFFSET).ok_or(
            MediaParserError::CorruptedData(MVHD_V0_TIMESCALE_OFFSET as u64),
         )?;
         let duration = read_u32_be(mvhd, MVHD_V0_DURATION_OFFSET).ok_or(
            MediaParserError::CorruptedData(MVHD_V0_DURATION_OFFSET as u64),
         )? as u64;
         Ok((timescale, duration))
      }
      1 => {
         if mvhd.len() < MVHD_V1_MIN_SIZE {
            return Err(MediaParserError::CorruptedData(mvhd.len() as u64));
         }
         let timescale = read_u32_be(mvhd, MVHD_V1_TIMESCALE_OFFSET).ok_or(
            MediaParserError::CorruptedData(MVHD_V1_TIMESCALE_OFFSET as u64),
         )?;
         let duration = read_u64_be(mvhd, MVHD_V1_DURATION_OFFSET).ok_or(
            MediaParserError::CorruptedData(MVHD_V1_DURATION_OFFSET as u64),
         )?;
         Ok((timescale, duration))
      }
      _ => Err(MediaParserError::InvalidFormat(format!(
         "unsupported mvhd version: {}",
         version
      ))),
   }
}

/// Extracts metadata tags from the ilst box.
fn extract_metadata_tags(moov_payload: &[u8]) -> Vec<types::Meta> {
   let mut values = Vec::new();

   // Navigate: moov -> udta -> meta -> ilst
   let udta_meta_path = [Mp4Box::Udta.bytes(), Mp4Box::Meta.bytes()];
   let Some(meta) = moov_payload.nav(&udta_meta_path) else {
      return values;
   };

   // Skip meta box flags (4 bytes)
   let meta_payload = if meta.len() >= 4 { &meta[4..] } else { meta };

   let ilst_path = [Mp4Box::Ilst.bytes()];
   let Some(ilst) = meta_payload.nav(&ilst_path) else {
      return values;
   };

   parse_ilst_entries(ilst, &mut values);
   values
}

/// Parses individual entries from the ilst box.
fn parse_ilst_entries(ilst: &[u8], values: &mut Vec<types::Meta>) {
   for (fourcc, payload) in iter_boxes(ilst) {
      let data_path = [Mp4Box::Data.bytes()];
      if let Some(data_box) = payload.nav(&data_path)
         && data_box.len() >= 8
      {
         let Some(dtype) = read_u32_be(data_box, 0) else {
            continue;
         };
         let raw = &data_box[8..];

         let maybe_text = match dtype {
            1 => decode_utf8(raw),           // UTF-8
            2 => decode_utf16_with_bom(raw), // UTF-16
            _ => None,                       // Non-text types ignored
         };

         if let Some(text) = maybe_text.and_then(|s| trim_null_and_whitespace(&s)) {
            let key = fourcc_to_key(fourcc);
            let name = tag_name(fourcc).to_string();
            values.push(types::Meta {
               key,
               name,
               value: text,
            });
         }
      }
   }
}

#[cfg(test)]
mod tests {
   use super::*;

   #[test]
   fn test_fourcc_to_key() {
      assert_eq!(fourcc_to_key([0xA9, b'n', b'a', b'm']), "@nam");
      assert_eq!(fourcc_to_key([0xA9, b'A', b'R', b'T']), "@ART");
      assert_eq!(fourcc_to_key(*b"cprt"), "cprt");
      assert_eq!(fourcc_to_key(*b"trkn"), "trkn");
   }

   #[test]
   fn test_tag_name_known() {
      assert_eq!(tag_name([0xA9, b'n', b'a', b'm']), "Title");
      assert_eq!(tag_name([0xA9, b'A', b'R', b'T']), "Artist");
      assert_eq!(tag_name(*b"cprt"), "Copyright");
   }

   #[test]
   fn test_tag_name_unknown() {
      assert_eq!(tag_name(*b"xxxx"), "Unknown");
   }
}
