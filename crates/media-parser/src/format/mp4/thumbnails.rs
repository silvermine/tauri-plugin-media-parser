//! # MP4 Thumbnail Extraction (TODO)
//!
//! This module will handle extraction of thumbnails/poster images from MP4 files.
//!
//! ## Sources for Thumbnails
//!
//! 1. **Embedded artwork** in metadata (`covr` atom in `ilst`)
//! 2. **Video keyframes** (I-frames from video track)
//! 3. **Chapter thumbnails** (if present)
//!
//! ## Box Structure for Artwork
//!
//! ```text
//! [moov]
//!   └── [udta]
//!       └── [meta]
//!           └── [ilst]
//!               └── [covr]
//!                   └── [data] - JPEG or PNG image data
//! ```
//!
//! ## Box Structure for Video Keyframes
//!
//! ```text
//! [moov]
//!   └── [trak] (handler_type = 'vide')
//!       └── [mdia]
//!           └── [minf]
//!               └── [stbl]
//!                   ├── [stss] - Sync sample table (keyframe indices)
//!                   ├── [stsz] - Sample sizes
//!                   └── [stco/co64] - Chunk offsets
//! [mdat] - Contains actual video frames
//! ```
