//! Decoding of raw subtitle samples into cue text.
//!
//! Each supported codec carries its text differently, so decoding is where a
//! malformed sample is separated from a malformed file: a failure that only
//! invalidates one track is reported apart from one that must fail the whole
//! request.

use super::super::atoms;
use crate::MediaParserError;
use std::char::decode_utf16;

#[derive(Debug)]
pub(super) enum DecodeSampleError {
   Track(MediaParserError),
   Fatal(MediaParserError),
}

pub(super) fn decode_sample(
   codec: &str,
   data: &[u8],
) -> std::result::Result<Option<String>, DecodeSampleError> {
   decode_sample_bounded(codec, data, None)
}

#[cfg(test)]
fn decode_sample_with_limit(
   codec: &str,
   data: &[u8],
   max_capacity: usize,
) -> std::result::Result<Option<String>, DecodeSampleError> {
   decode_sample_bounded(codec, data, Some(max_capacity))
}

fn decode_sample_bounded(
   codec: &str,
   data: &[u8],
   capacity_limit: Option<usize>,
) -> std::result::Result<Option<String>, DecodeSampleError> {
   if !matches!(codec, "tx3g" | "wvtt" | "stpp" | "text") {
      return unsupported_codec(codec, capacity_limit);
   }

   let mut output = sample_text_builder(data, capacity_limit)?;
   decode_supported_sample(codec, data, &mut output)?;
   Ok(output.finish())
}

fn sample_text_builder(
   data: &[u8],
   capacity_limit: Option<usize>,
) -> std::result::Result<TextBuilder, DecodeSampleError> {
   let natural_ceiling = data.len().checked_mul(3).ok_or_else(capacity_error)?;
   let max_capacity = capacity_limit.map_or(natural_ceiling, |limit| limit.min(natural_ceiling));
   Ok(TextBuilder::new(max_capacity))
}

fn decode_supported_sample(
   codec: &str,
   data: &[u8],
   output: &mut TextBuilder,
) -> std::result::Result<(), DecodeSampleError> {
   match codec {
      "tx3g" | "text" => {
         let length_bytes = data.get(..2).ok_or_else(invalid_subtitle)?;
         let declared_length = usize::from(u16::from_be_bytes([length_bytes[0], length_bytes[1]]));
         let text_end = 2_usize
            .checked_add(declared_length)
            .ok_or_else(invalid_subtitle)?;
         let text = data.get(2..text_end).ok_or_else(invalid_subtitle)?;
         output.append_decoded(text)?;
      }
      "wvtt" => decode_wvtt(data, output)?,
      "stpp" => {
         output.append_decoded(data)?;
      }
      _ => unreachable!("unsupported codecs return before sample text allocation"),
   }
   Ok(())
}

#[cfg(test)]
fn decode_sample_with_reservation_count(
   codec: &str,
   data: &[u8],
) -> std::result::Result<(Option<String>, usize), DecodeSampleError> {
   let mut output = sample_text_builder(data, None)?;
   decode_supported_sample(codec, data, &mut output)?;
   let reservation_count = output.reservation_count();
   Ok((output.finish(), reservation_count))
}

fn decode_wvtt(
   data: &[u8],
   output: &mut TextBuilder,
) -> std::result::Result<(), DecodeSampleError> {
   walk_boxes(data, |fourcc, payload| match &fourcc {
      b"payl" => append_wvtt_payload(output, payload),
      b"vttc" => walk_boxes(payload, |child_fourcc, child_payload| {
         if &child_fourcc == b"payl" {
            append_wvtt_payload(output, child_payload)?;
         }
         Ok(())
      }),
      _ => Ok(()),
   })
}

fn walk_boxes(
   data: &[u8],
   mut visit: impl FnMut([u8; 4], &[u8]) -> std::result::Result<(), DecodeSampleError>,
) -> std::result::Result<(), DecodeSampleError> {
   let mut offset = 0_usize;
   while offset < data.len() {
      let parsed = atoms::read_box(data, offset).ok_or_else(invalid_subtitle)?;
      let next = offset
         .checked_add(parsed.total_size)
         .filter(|next| *next > offset && *next <= data.len())
         .ok_or_else(invalid_subtitle)?;
      visit(parsed.fourcc, parsed.payload)?;
      offset = next;
   }
   Ok(())
}

fn append_wvtt_payload(
   output: &mut TextBuilder,
   payload: &[u8],
) -> std::result::Result<(), DecodeSampleError> {
   let previous_len = output.len();
   if output.append_decoded(payload)? && previous_len != 0 {
      output.insert(previous_len, '\n')?;
   }
   Ok(())
}

struct TextBuilder {
   text: String,
   max_capacity: usize,
   #[cfg(test)]
   reservation_count: usize,
}

impl TextBuilder {
   fn new(max_capacity: usize) -> Self {
      Self {
         text: String::new(),
         max_capacity,
         #[cfg(test)]
         reservation_count: 0,
      }
   }

   fn len(&self) -> usize {
      self.text.len()
   }

   #[cfg(test)]
   fn reservation_count(&self) -> usize {
      self.reservation_count
   }

   fn finish(self) -> Option<String> {
      if self.text.is_empty() {
         None
      } else {
         Some(self.text)
      }
   }

   fn append_decoded(&mut self, data: &[u8]) -> std::result::Result<bool, DecodeSampleError> {
      if let Some(bytes) = data.strip_prefix(&[0xfe, 0xff]) {
         return self.append_utf16(bytes, Endian::Big);
      }
      if let Some(bytes) = data.strip_prefix(&[0xff, 0xfe]) {
         return self.append_utf16(bytes, Endian::Little);
      }
      if let Ok(text) = std::str::from_utf8(data) {
         let trimmed = text.trim_matches(is_trim_character);
         if trimmed.is_empty() {
            return Ok(false);
         }
         self.append_str(trimmed)?;
         return Ok(true);
      }
      if !data.len().is_multiple_of(2) {
         return Err(invalid_subtitle());
      }
      self.append_utf16(data, Endian::Big)
   }

   fn append_utf16(
      &mut self,
      data: &[u8],
      endian: Endian,
   ) -> std::result::Result<bool, DecodeSampleError> {
      if !data.len().is_multiple_of(2) {
         return Err(invalid_subtitle());
      }

      let start = self.text.len();
      let code_units = data.len() / 2;
      let conservative_capacity = code_units.checked_mul(3).ok_or_else(capacity_error)?;
      self.reserve(conservative_capacity)?;

      let units = data.chunks_exact(2).map(|bytes| match endian {
         Endian::Big => u16::from_be_bytes([bytes[0], bytes[1]]),
         Endian::Little => u16::from_le_bytes([bytes[0], bytes[1]]),
      });
      for character in decode_utf16(units) {
         self.text.push(character.map_err(|_| invalid_subtitle())?);
      }

      Ok(self.trim_from(start))
   }

   fn append_str(&mut self, value: &str) -> std::result::Result<(), DecodeSampleError> {
      self.reserve(value.len())?;
      self.text.push_str(value);
      Ok(())
   }

   fn insert(
      &mut self,
      index: usize,
      character: char,
   ) -> std::result::Result<(), DecodeSampleError> {
      self.reserve(character.len_utf8())?;
      self.text.insert(index, character);
      Ok(())
   }

   fn reserve(&mut self, additional: usize) -> std::result::Result<(), DecodeSampleError> {
      const MIN_GROWTH: usize = 8;

      let target = self
         .text
         .len()
         .checked_add(additional)
         .ok_or_else(capacity_error)?;
      if target > self.max_capacity {
         return Err(capacity_error());
      }
      if self.text.capacity() >= target {
         return Ok(());
      }

      let doubled = self
         .text
         .capacity()
         .checked_mul(2)
         .unwrap_or(self.max_capacity);
      let desired_capacity = doubled.max(MIN_GROWTH).max(target).min(self.max_capacity);
      let reservation = desired_capacity
         .checked_sub(self.text.len())
         .ok_or_else(capacity_error)?;
      #[cfg(test)]
      {
         self.reservation_count += 1;
      }
      self
         .text
         .try_reserve_exact(reservation)
         .map_err(|_| capacity_error())
   }

   fn trim_from(&mut self, start: usize) -> bool {
      let segment = &self.text[start..];
      let Some((first, _)) = segment
         .char_indices()
         .find(|(_, character)| !is_trim_character(*character))
      else {
         self.text.truncate(start);
         return false;
      };
      let last = segment
         .char_indices()
         .rev()
         .find(|(_, character)| !is_trim_character(*character))
         .map(|(index, character)| index + character.len_utf8())
         .unwrap_or(first);

      self.text.truncate(start + last);
      if first != 0 {
         drop(self.text.drain(start..start + first));
      }
      true
   }
}

#[derive(Clone, Copy)]
enum Endian {
   Big,
   Little,
}

fn is_trim_character(character: char) -> bool {
   character == '\0' || character.is_whitespace()
}

fn invalid_subtitle() -> DecodeSampleError {
   DecodeSampleError::Track(MediaParserError::SubtitleError(
      "invalid MP4 subtitle sample".to_owned(),
   ))
}

fn capacity_error() -> DecodeSampleError {
   DecodeSampleError::Fatal(MediaParserError::Other(
      "MP4 subtitle text capacity exceeded".to_owned(),
   ))
}

fn unsupported_codec(
   codec: &str,
   capacity_limit: Option<usize>,
) -> std::result::Result<Option<String>, DecodeSampleError> {
   const MAX_CODEC_IDENTIFIER_BYTES: usize = 16;

   let mut end = 0_usize;
   for (index, character) in codec.char_indices() {
      let next = index + character.len_utf8();
      if next > MAX_CODEC_IDENTIFIER_BYTES {
         break;
      }
      end = next;
   }
   if end > capacity_limit.unwrap_or(MAX_CODEC_IDENTIFIER_BYTES) {
      return Err(capacity_error());
   }

   let mut identifier = String::new();
   identifier
      .try_reserve_exact(end)
      .map_err(|_| capacity_error())?;
   identifier.push_str(&codec[..end]);
   Err(DecodeSampleError::Track(
      MediaParserError::UnsupportedCodec(identifier),
   ))
}

#[cfg(test)]
mod tests {
   use super::{
      DecodeSampleError, decode_sample, decode_sample_with_limit,
      decode_sample_with_reservation_count,
   };
   use crate::MediaParserError;
   use rstest::rstest;

   fn mp4_box(fourcc: &[u8; 4], payload: &[u8]) -> Vec<u8> {
      let size = 8_u32 + u32::try_from(payload.len()).unwrap();
      let mut data = Vec::new();
      data.extend_from_slice(&size.to_be_bytes());
      data.extend_from_slice(fourcc);
      data.extend_from_slice(payload);
      data
   }

   fn length_prefixed_text(text: &[u8], trailing: &[u8]) -> Vec<u8> {
      let mut data = Vec::new();
      data.extend_from_slice(&u16::try_from(text.len()).unwrap().to_be_bytes());
      data.extend_from_slice(text);
      data.extend_from_slice(trailing);
      data
   }

   fn assert_subtitle_error(result: Result<Option<String>, DecodeSampleError>) {
      assert!(matches!(
         result,
         Err(DecodeSampleError::Track(MediaParserError::SubtitleError(_)))
      ));
   }

   fn assert_fatal(result: Result<Option<String>, DecodeSampleError>) {
      let Err(DecodeSampleError::Fatal(error)) = result else {
         panic!("expected a fatal subtitle decode error");
      };
      assert!(matches!(error, MediaParserError::Other(_)));
   }

   fn wvtt_payloads_among_ignored_boxes() -> Vec<u8> {
      let mut children = mp4_box(b"payl", b"second");
      children.extend(mp4_box(b"iden", b"ignored"));
      children.extend(mp4_box(b"payl", b" third "));

      let mut sample = mp4_box(b"payl", b" first ");
      sample.extend(mp4_box(b"junk", b"ignored"));
      sample.extend(mp4_box(b"vttc", &children));
      sample.extend(mp4_box(b"payl", b"fourth"));
      sample
   }

   fn wvtt_unknown_well_formed_boxes() -> Vec<u8> {
      let nested_unknown = mp4_box(b"vttc", &mp4_box(b"sttg", b"line:0"));
      let mut sample = mp4_box(b"free", b"anything");
      sample.extend(nested_unknown);
      sample
   }

   #[rstest]
   #[case::tx3g_strict_utf8("tx3g", length_prefixed_text(" café ".as_bytes(), &[]), Some("café"))]
   #[case::tx3g_bom_utf16_big_endian(
      "tx3g",
      length_prefixed_text(&[0xfe, 0xff, 0x00, b' ', 0x00, b'O', 0x00, b'K', 0x00, b' '], &[]),
      Some("OK"),
   )]
   #[case::tx3g_bom_utf16_little_endian(
      "tx3g",
      length_prefixed_text(&[0xff, 0xfe, b' ', 0x00, b'O', 0x00, b'K', 0x00, b' ', 0x00], &[]),
      Some("OK"),
   )]
   #[case::tx3g_big_endian_bom_without_text("tx3g", length_prefixed_text(&[0xfe, 0xff], &[]), None)]
   #[case::tx3g_little_endian_bom_without_text(
      "tx3g",
      length_prefixed_text(&[0xff, 0xfe], &[]),
      None,
   )]
   #[case::tx3g_utf16_supplementary_plane_character(
      "tx3g",
      length_prefixed_text(&[0xfe, 0xff, 0xd8, 0x3d, 0xde, 0x00], &[]),
      Some("😀"),
   )]
   #[case::tx3g_bomless_utf16_big_endian_fallback(
      "tx3g",
      length_prefixed_text(&[0x00, 0x20, 0x00, 0xe9, 0x00, 0x20], &[]),
      Some("é"),
   )]
   #[case::tx3g_trailing_style_records(
      "tx3g",
      length_prefixed_text(b"caption", &[0xff, 0xfe, 0xfd, 0xfc]),
      Some("caption"),
   )]
   #[case::tx3g_empty("tx3g", vec![0, 0], None)]
   #[case::tx3g_trimmed_empty("tx3g", length_prefixed_text(b" \t\r\n\0 ", &[]), None)]
   #[case::wvtt_direct_payload("wvtt", mp4_box(b"payl", b" hello "), Some("hello"))]
   #[case::wvtt_payload_nested_in_vttc(
      "wvtt",
      mp4_box(b"vttc", &mp4_box(b"payl", b"nested")),
      Some("nested"),
   )]
   #[case::wvtt_non_gap_payloads_joined_in_source_order(
      "wvtt",
      wvtt_payloads_among_ignored_boxes(),
      Some("first\nsecond\nthird\nfourth")
   )]
   #[case::wvtt_unknown_well_formed_boxes("wvtt", wvtt_unknown_well_formed_boxes(), None)]
   #[case::wvtt_empty("wvtt", vec![], None)]
   #[case::wvtt_empty_vttc("wvtt", mp4_box(b"vttc", &[]), None)]
   #[case::wvtt_whitespace_payload("wvtt", mp4_box(b"payl", b" \0\t "), None)]
   #[case::stpp_strict_utf8("stpp", b" <p>cue</p>\0 ".to_vec(), Some("<p>cue</p>"))]
   #[case::text_bom_utf16_little_endian(
      "text",
      length_prefixed_text(&[0xff, 0xfe, b'O', 0, b'K', 0], &[]),
      Some("OK"),
   )]
   #[case::text_trimmed_empty("text", length_prefixed_text(b" \n\0\t", &[]), None)]
   #[case::text_multibyte_unicode_whitespace_at_both_boundaries(
      "text",
      length_prefixed_text("\u{2003}é\u{3000}".as_bytes(), &[]),
      Some("é"),
   )]
   #[case::text_trailing_style_atom_with_non_utf8_bytes(
      "text",
      length_prefixed_text(b"Fourteen bytes", &mp4_box(b"styl", &[0x80])),
      Some("Fourteen bytes"),
   )]
   fn decodes_text_or_a_gap(
      #[case] codec: &str,
      #[case] data: Vec<u8>,
      #[case] expected: Option<&str>,
   ) {
      assert_eq!(decode_sample(codec, &data).unwrap().as_deref(), expected);
   }

   #[rstest]
   #[case::tx3g_isolated_low_surrogate(
      "tx3g",
      length_prefixed_text(&[0xfe, 0xff, 0xdc, 0x00], &[]),
   )]
   #[case::tx3g_unpaired_high_surrogate_at_the_end(
      "tx3g",
      length_prefixed_text(&[0xfe, 0xff, 0xd8, 0x00], &[]),
   )]
   #[case::tx3g_high_surrogate_before_a_non_surrogate(
      "tx3g",
      length_prefixed_text(&[0xfe, 0xff, 0xd8, 0x00, 0x00, b'A'], &[]),
   )]
   #[case::tx3g_missing_length("tx3g", vec![])]
   #[case::tx3g_truncated_length("tx3g", vec![0])]
   #[case::tx3g_text_shorter_than_declared("tx3g", vec![0, 3, b'a', b'b'])]
   #[case::tx3g_invalid_utf8("tx3g", length_prefixed_text(&[0xff], &[]))]
   #[case::tx3g_invalid_bomless_utf16("tx3g", length_prefixed_text(&[0xd8, 0x00], &[]))]
   #[case::tx3g_odd_length_bom_utf16("tx3g", length_prefixed_text(&[0xfe, 0xff, 0x00], &[]))]
   #[case::wvtt_nested_zero_size_box(
      "wvtt",
      mp4_box(b"vttc", &[0, 0, 0, 0, b'p', b'a', b'y', b'l']),
   )]
   #[case::wvtt_nested_truncated_box_header("wvtt", mp4_box(b"vttc", &[1, 2, 3]))]
   #[case::stpp_invalid_utf8("stpp", vec![0xff])]
   #[case::text_shorter_than_declared("text", vec![0, 3, b'a', b'b'])]
   fn rejects_malformed_samples(#[case] codec: &str, #[case] data: Vec<u8>) {
      assert_subtitle_error(decode_sample(codec, &data));
   }

   fn payl_with_trailing_bytes() -> Vec<u8> {
      let mut trailing = mp4_box(b"payl", b"ok");
      trailing.extend_from_slice(&[1, 2, 3]);
      trailing
   }

   #[rstest]
   #[case::size_past_end(vec![0, 0, 0, 12, b'p', b'a', b'y', b'l'])]
   #[case::zero_size(vec![0, 0, 0, 0, b'p', b'a', b'y', b'l'])]
   #[case::size_below_header(vec![0, 0, 0, 4, b'p', b'a', b'y', b'l'])]
   #[case::largesize_past_end(vec![
      0, 0, 0, 1, b'p', b'a', b'y', b'l', 0xff, 0xff, 0xff, 0xff, 0xff, 0xff, 0xff, 0xff,
   ])]
   #[case::trailing_bytes(payl_with_trailing_bytes())]
   fn wvtt_rejects_malformed_top_level_box_framing(#[case] data: Vec<u8>) {
      assert_subtitle_error(decode_sample("wvtt", &data));
   }

   #[test]
   fn unsupported_codec_is_a_track_local_error() {
      let Err(DecodeSampleError::Track(MediaParserError::UnsupportedCodec(codec))) =
         decode_sample("c608", b"caption")
      else {
         panic!("expected a track-local unsupported codec error");
      };
      assert_eq!(codec, "c608");
   }

   fn two_wvtt_payloads() -> Vec<u8> {
      let mut sample = mp4_box(b"payl", b"ab");
      sample.extend(mp4_box(b"payl", b"cd"));
      sample
   }

   #[rstest]
   #[case::unsupported_codec("c608", b"caption".to_vec(), 3)]
   #[case::utf8("text", length_prefixed_text(b"valid", &[]), 4)]
   #[case::utf16("text", length_prefixed_text(&[0xfe, 0xff, 0x4f, 0x60], &[]), 2)]
   #[case::wvtt_joining("wvtt", two_wvtt_payloads(), 4)]
   fn capacity_refusal_is_fatal(#[case] codec: &str, #[case] data: Vec<u8>, #[case] limit: usize) {
      assert_fatal(decode_sample_with_limit(codec, &data, limit));
   }

   #[test]
   fn wvtt_many_tiny_payloads_use_logarithmically_bounded_growth() {
      const PAYLOAD_COUNT: usize = 512;
      let mut sample = Vec::new();
      for _ in 0..PAYLOAD_COUNT {
         sample.extend(mp4_box(b"payl", b"x"));
      }

      let (output, reservation_count) =
         decode_sample_with_reservation_count("wvtt", &sample).unwrap();
      let output = output.unwrap();

      assert_eq!(output.len(), PAYLOAD_COUNT * 2 - 1);
      assert!(
         reservation_count <= 16,
         "{} fallible reservation calls for {PAYLOAD_COUNT} payloads",
         reservation_count
      );
   }
}
