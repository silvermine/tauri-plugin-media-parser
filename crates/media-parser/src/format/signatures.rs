//! # Format Signatures
//!
//! Format detection metadata for all supported formats.
//!
//! Canonical definitions of format names, file extensions,
//! byte markers, and MIME types.

use super::FormatSignature;

/// MP3 format signature.
///
/// Detects MP3 files by:
/// - ID3v2 tag marker ("ID3" at offset 0)
/// - MPEG frame sync bytes (0xFFxx patterns)
pub const MP3: FormatSignature = FormatSignature {
   name: "MP3",
   extensions: &["mp3"],
   markers: &[
      (0, b"ID3"),        // ID3v2 tag
      (0, &[0xFF, 0xFB]), // MP3 frame sync (MPEG1 Layer3)
      (0, &[0xFF, 0xFA]), // MP3 frame sync (MPEG1 Layer3)
      (0, &[0xFF, 0xF3]), // MP3 frame sync (MPEG2 Layer3)
      (0, &[0xFF, 0xF2]), // MP3 frame sync (MPEG2 Layer3)
   ],
   mime_types: &["audio/mpeg", "audio/mp3"],
};

/// MP4/M4A/MOV format signature.
///
/// Detects MP4 container formats by:
/// - "ftyp" box marker at offset 4
pub const MP4: FormatSignature = FormatSignature {
   name: "MP4/M4A/MOV",
   extensions: &["mp4", "m4a", "m4v", "mov", "m4b", "m4p"],
   markers: &[(4, b"ftyp")],
   mime_types: &["video/mp4", "audio/mp4", "audio/x-m4a", "video/quicktime"],
};
