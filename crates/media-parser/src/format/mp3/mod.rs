//! # MP3 Format Implementation
//!
//! Parser for MP3 audio files with ID3v2 metadata and duration calculation.
//!
//! ## Module Structure
//!
//! ```text
//! mp3/
//! ├── mod.rs          # Format registration and public API
//! ├── metadata.rs     # ID3v2 tag parsing
//! ├── duration.rs     # Duration calculation (CBR/VBR strategies)
//! ├── frame.rs        # MPEG frame header parsing
//! └── tables.rs       # MPEG lookup tables
//! ```
//!
//! ## ID3v2 Structure
//!
//! ```text
//! [ID3 Header] - 10 bytes
//!   ├── "ID3" marker (3 bytes)
//!   ├── Version (2 bytes)
//!   ├── Flags (1 byte)
//!   └── Size (4 bytes, syncsafe)
//! [ID3 Frames] - variable
//!   ├── TIT2 - Title
//!   ├── TPE1 - Artist
//!   ├── TALB - Album
//!   ├── TYER - Year
//!   ├── TRCK - Track number
//!   └── ...
//! [Audio Data] - MP3 frames
//! ```
//!
//! ## Duration Calculation
//!
//! Duration is calculated using one of these options:
//! - VBR: Parses Xing/VBRI header for exact frame count
//! - CBR: Calculates from file size and bitrate

pub mod duration;
pub mod frame;
pub mod metadata;
pub mod tables;
pub mod tags;

use crate::Result;
use crate::format::{AsyncParser, Format};
use crate::stream::StreamReader;
use crate::types::Metadata;
use std::future::Future;
use std::pin::Pin;

/// MP3 format signature for detection.
pub use crate::format::signatures::MP3 as SIGNATURE;

/// Parser entry point for the registry.
fn parse(reader: &dyn StreamReader) -> Pin<Box<dyn Future<Output = Result<Metadata>> + Send + '_>> {
   Box::pin(parse_mp3(reader))
}

/// MP3 format definition registered in the global table.
pub static FORMAT: Format = Format::new(SIGNATURE, parse as AsyncParser);

/// Main parsing function.
async fn parse_mp3(reader: &dyn StreamReader) -> Result<Metadata> {
   metadata::read_metadata(reader).await
}

// Re-export public types
pub use duration::{
   AutoStrategy, CbrStrategy, Duration, DurationMethod, DurationStrategy, VbrHeaderType, VbrInfo,
   VbrStrategy, calculate_duration, calculate_duration_with_strategy, parse_vbr_header,
};
pub use frame::{FrameHeader, FrameParseResult, MAX_SYNC_SEARCH, find_first_frame};
pub use metadata::read_metadata;
pub use tables::{MpegLayer, MpegVersion};
pub use tags::{frame_id_to_key, frame_name};
