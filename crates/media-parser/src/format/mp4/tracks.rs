//! MP4 track metadata extraction.
//!
//! Reads information from each `trak` box without touching media samples.

use super::atoms::{
   Mp4Nav, audio_params, find_and_read_moov_box, fourcc_string, iter_boxes, parse_hdlr,
   parse_mdhd, parse_stsd, parse_tkhd, stts_sample_count, visual_dimensions,
};
use crate::Result;
use crate::errors::MediaParserError;
use crate::helpers::read_u32_be;
use crate::stream::StreamReader;
use crate::types::{
   AudioTrackMeta, BaseTrackMeta, SubtitleTrackMeta, TrackType, UnknownTrackMeta, VideoTrackMeta,
};
use std::collections::HashMap;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum TrackKind {
   Video,
   Audio,
   Subtitle,
   Unknown,
}

enum SampleEntry {
   Visual {
      width: Option<u32>,
      height: Option<u32>,
   },
   Audio {
      channels: Option<u16>,
      sample_rate: Option<u32>,
   },
   None,
}

/// Reads all MP4 tracks from the `moov/trak` boxes.
pub async fn read_tracks(reader: &dyn StreamReader) -> Result<Vec<TrackType>> {
   let moov_data = find_and_read_moov_box(reader).await?;
   let moov_payload = if moov_data.len() >= 8 && &moov_data[4..8] == b"moov" {
      &moov_data[8..]
   } else {
      &moov_data
   };

   let mut tracks = Vec::new();
   for (fourcc, trak) in iter_boxes(moov_payload) {
      if &fourcc != b"trak" {
         continue;
      }

      // Best-effort: skip a malformed trak (missing the spec-required
      // tkhd/mdia/mdhd) instead of failing the whole file, so one bad track
      // doesn't drop every other track.
      match parse_trak(trak) {
         Ok(track) => tracks.push(track),
         Err(e) => log::warn!("skipping malformed trak: {e}"),
      }
   }

   Ok(tracks)
}

fn parse_trak(trak: &[u8]) -> Result<TrackType> {
   let tkhd = trak
      .nav(&[*b"tkhd"])
      .and_then(parse_tkhd)
      .ok_or_else(|| MediaParserError::InvalidFormat("trak missing tkhd box".to_string()))?;

   let mdia = trak
      .nav(&[*b"mdia"])
      .ok_or_else(|| MediaParserError::InvalidFormat("trak missing mdia box".to_string()))?;

   let mdhd = mdia
      .nav(&[*b"mdhd"])
      .and_then(parse_mdhd)
      .ok_or_else(|| MediaParserError::InvalidFormat("trak missing mdhd box".to_string()))?;

   let handler = mdia
      .nav(&[*b"hdlr"])
      .and_then(parse_hdlr)
      .unwrap_or(*b"    ");
   let kind = classify_handler(handler);

   let stbl = mdia.nav(&[*b"minf", *b"stbl"]);
   let stsd = stbl
      .and_then(|stbl| stbl.nav(&[*b"stsd"]))
      .and_then(|stsd| {
         parse_stsd(stsd, |payload| match kind {
            TrackKind::Video => {
               let (width, height) = visual_dimensions(payload);
               SampleEntry::Visual { width, height }
            }
            TrackKind::Audio => {
               let (channels, sample_rate) = audio_params(payload);
               SampleEntry::Audio {
                  channels,
                  sample_rate,
               }
            }
            _ => SampleEntry::None,
         })
      });

   let mut properties = HashMap::new();
   properties.insert("handler_type".to_string(), fourcc_string(handler));
   properties.insert("tkhd_duration".to_string(), tkhd.duration.to_string());
   if let Some(stsd) = &stsd {
      properties.insert(
         "sample_entry_count".to_string(),
         stsd.entry_count.to_string(),
      );
   }
   add_sample_table_properties(stbl, &mut properties);

   let base = BaseTrackMeta {
      id: tkhd.id,
      codec: stsd
         .as_ref()
         .map(|s| s.codec.clone())
         .unwrap_or_else(|| "unknown".to_string()),
      language: mdhd.language.map(language_string),
      timescale: mdhd.timescale,
      duration: mdhd.duration,
      properties,
   };

   match kind {
      TrackKind::Video => {
         let (width, height) = match stsd.as_ref().map(|s| &s.entry) {
            Some(SampleEntry::Visual { width, height }) => (*width, *height),
            _ => (None, None),
         };
         Ok(TrackType::Video(VideoTrackMeta {
            base,
            width: width.unwrap_or(tkhd.width),
            height: height.unwrap_or(tkhd.height),
         }))
      }
      TrackKind::Audio => {
         let (channels, sample_rate) = match stsd.as_ref().map(|s| &s.entry) {
            Some(SampleEntry::Audio {
               channels,
               sample_rate,
            }) => (*channels, *sample_rate),
            _ => (None, None),
         };
         Ok(TrackType::Audio(AudioTrackMeta {
            base,
            channels: channels.unwrap_or(0),
            sample_rate: sample_rate.unwrap_or(0),
         }))
      }
      TrackKind::Subtitle => Ok(TrackType::Subtitle(SubtitleTrackMeta { base })),
      TrackKind::Unknown => Ok(TrackType::Unknown(UnknownTrackMeta { base })),
   }
}

fn add_sample_table_properties(stbl: Option<&[u8]>, properties: &mut HashMap<String, String>) {
   let Some(stbl) = stbl else {
      return;
   };

   if let Some(stts) = stbl.nav(&[*b"stts"]) {
      if let Some(entry_count) = read_u32_be(stts, 4) {
         properties.insert("stts_entry_count".to_string(), entry_count.to_string());
      }
      if let Some(sample_count) = stts_sample_count(stts) {
         properties.insert("sample_count".to_string(), sample_count.to_string());
      }
   }

   if let Some(stsz) = stbl.nav(&[*b"stsz"]) {
      if let Some(fixed_sample_size) = read_u32_be(stsz, 4) {
         properties.insert(
            "fixed_sample_size".to_string(),
            fixed_sample_size.to_string(),
         );
      }
      if let Some(sample_count) = read_u32_be(stsz, 8) {
         properties.insert("stsz_sample_count".to_string(), sample_count.to_string());
      }
   }
}

fn classify_handler(handler: [u8; 4]) -> TrackKind {
   match &handler {
      b"vide" => TrackKind::Video,
      b"soun" => TrackKind::Audio,
      b"sbtl" | b"subt" | b"text" | b"clcp" => TrackKind::Subtitle,
      _ => TrackKind::Unknown,
   }
}

fn language_string(language: [u8; 3]) -> String {
   String::from_utf8_lossy(&language).into_owned()
}
