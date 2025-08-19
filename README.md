# media-parser

## Overview

The `media-parser` crate provides an API for getting metadata, tracks, subtitles
and frames from a local or remote MP4 media file.


## Public API

### Constructor

The constructor requires a `StreamReader` to read media files.

```rust
impl<R: StreamReader> MediaParser<R> {
   /// Create a new media parser with the provided stream reader
   pub fn new(reader: R) -> Self {
      // ...
   }
}
```

## 1. Get Metadata

Get metadata including title, artist, album, copyright, and other well-known properties.

### Metadata - Types

```rust
#[derive(Debug, Clone, PartialEq)]
pub struct Metadata {
   pub key: String,
   pub value: String
}

pub trait MetadataExt {
   fn title(&self) -> Option<&str>;
   fn artist(&self) -> Option<&str>;
   // ...
}

impl MetadataExt for Vec<Metadata> {
   // ...
}
```

### Metadata - API Methods

```rust
impl<R: StreamReader> MediaParser<R> {
   pub async fn metadata(&mut self) -> Result<Vec<Metadata>> {
      // ...
   }
}
```

### Metadata - Usage Example

```rust
use media_parser::{MediaParser, FileStreamReader};

#[tokio::main]
async fn main() -> media_parser::Result<()> {
   let reader = FileStreamReader::new("video.mp4");
   let parser = MediaParser::new(reader);
   
   let metadata = parser.metadata().await?;

   // Get all metadata keys.
   for meta in &metadata {
      println!("Key: {}", meta.key);
      println!("Value: {:?}", meta.value);
   }

   // Get "well-known" metadata keys.
   println!("Title: {:?}", metadata.title());
   println!("Artist: {:?}", metadata.artist());
   println!("Album: {:?}", metadata.album());
   println!("Copyright: {:?}", metadata.copyright());
   println!("Duration: {:?}", metadata.duration());
   
   Ok(())
}
```

## 2. Get Tracks

Get list of video, audio, and subtitle tracks including track properties.

### Tracks - Types

```rust
#[derive(Debug, Clone, PartialEq)]
pub enum TrackType {
   Video(VideoTrackMeta),
   Audio(AudioTrackMeta),
   Subtitle(SubtitleTrackMeta),
   Unknown(UnknownTrackMeta)
}

#[derive(Debug, Clone, PartialEq)]
pub struct BaseTrackMeta {
   pub id: u32,
   pub codec: String,
   pub language: Option<String>,
   pub timescale: u32,  // Number of time units per second
   pub duration: u64,   // Raw duration in timescale units
   pub properties: HashMap<String, String>   // Additional properties
}

#[derive(Debug, Clone, PartialEq)]
pub struct VideoTrackMeta {
   pub base: BaseTrackMeta,
   pub width: u32,
   pub height: u32,
   pub sample_durations: Option<Vec<u32>> // Optional. Per-frame durations for VFR
}

#[derive(Debug, Clone, PartialEq)]
pub struct AudioTrackMeta {
   pub base: BaseTrackMeta,
   pub channels: u16,
   pub sample_rate: u32,
   pub sample_sizes: Option<Vec<u32>>, // Optional. Size of each sample in bytes for VBR.
}

#[derive(Debug, Clone, PartialEq)]
pub struct SubtitleTrackMeta {
   pub base: BaseTrackMeta
}

#[derive(Debug, Clone, PartialEq)]
pub struct UnknownTrackMeta {
   pub base: BaseTrackMeta
}
```

### Tracks - API Methods

```rust
impl<R: StreamReader> MediaParser<R> {
   pub async fn tracks(&mut self) -> Result<Vec<TrackType>> {
      // ...
   }
}
```

### Tracks - Usage Example

```rust
use media_parser::{MediaParser, FileStreamReader, TrackType};

#[tokio::main]
async fn main() -> media_parser::Result<()> {
   let reader = FileStreamReader::new("video.mp4");
   let parser = MediaParser::new(reader);
   
   for track in parser.tracks().await? {
      match track {
         TrackType::Video(t) => {
            println!("Video #{:?} {}x{} {:?}", t.codec, t.frame_width, t.frame_height, t.frame_rate);
         }
         TrackType::Audio(t) => {
            println!("Audio #{:?} {}ch @{}Hz", t.codec, t.channels, t.sample_rate);
         }
         TrackType::Subtitle(t) => {
            println!("Subtitle #{:?} {:?}", t.codec, t.language);
         }
         TrackType::Unknown(t) => {
            println!("Unknown #{:?} {:?}", t.codec);
         }
      }
   }
    
    Ok(())
}
```

## 3. Get Subtitles

Get subtitle tracks and and timed text filtered by track ID or language.

### Subtitles - Types

```rust
#[derive(Debug, Clone)]
   pub enum TrackFilter {
   /// Filter by track ID
   TrackID(u32),
   /// Filter by language (e.g., "eng", "spa")
   Language(String)
}

#[derive(Debug, Clone, PartialEq)]
pub struct SubtitleTrack {
   pub base: BaseTrackMeta,
   pub subtitles: Vec<SubtitleText>
}

#[derive(Debug, Clone, PartialEq)]
pub struct SubtitleText {
   pub id: u32,
   pub start_time: Duration,
   pub end_time: Duration,
   pub text: String
}
```

### Subtitles - API Methods

```rust
impl<R: StreamReader> MediaParser<R> {
   pub async fn subtitles(&mut self, filter: Option<TrackFilter>) -> Result<Vec<SubtitleTrack>> {
      // ...
   }
}
```

### Subtitles - Usage Example

```rust
use media_parser::{MediaParser, FileStreamReader, TrackFilter};

#[tokio::main]
async fn main() -> media_parser::Result<()> {
   let reader = FileStreamReader::new("video.mp4");
   let parser = MediaParser::new(reader);

   // Get all subtitle tracks
   let tracks = parser.subtitles(None).await?;
   for track in &tracks {
      println!("Subtitle track ID {}", track.id);
      println!("Subtitle track language {}", track.language);

      for subtitle in &track.subtitles {
         println!("[{:?}-{:?}] {}", 
            subtitle.start_time, 
            subtitle.end_time, 
            subtitle.text);
      }
   }

   // Filter by track ID
   let tracks = parser.subtitles(TrackFilter::TrackID(1)).await?;

   // Filter by language
   let tracks = parser.subtitles(TrackFilter::Language("eng")).await?;
    
    Ok(())
}
```

## 4. Get Frames

Get frames at specific timestamps in native pixel format (YUV, RGB, etc.).

### Frames - Types

```rust
#[derive(Debug, Clone)]
pub struct Frame {
   pub track_id: u32,
   pub width: u32,
   pub height: u32,
   pub timestamp: Duration,
   pub format: PixelFormat,
   pub data: Vec<u8>,
   pub strides: Option<Vec<usize>> // Bytes per row for each plane
}

/// Supported pixel formats
#[derive(Debug, Clone, PartialEq)]
pub enum PixelFormat {
   /// YUV 4:2:0 planar format
   Yuv420p,
   /// YUV 4:2:2 planar format
   Yuv422p,
   /// YUV 4:4:4 planar format
   Yuv444p,
   /// RGB 24-bit format
   Rgb24,
   /// RGBA 32-bit format
   Rgba
}
```

### Frames - API Methods

```rust
impl<R: StreamReader> MediaParser<R> {
   // Get single frame
   pub async fn frame(&mut self, track_id: u32, timestamp: Duration) -> Result<Frame> {
      // ...
   }
   
   /// Get multiple frames
   pub async fn frames(&mut self, track_id: u32, timestamps: &[Duration]) -> Result<Vec<Frame>> {
      // ...
   }
}
```

### Frames - Usage Example

#### Single Frame

```rust
use media_parser::{MediaParser, FileStreamReader};
use std::time::Duration;

#[tokio::main]
async fn main() -> media_parser::Result<()> {
   let reader = FileStreamReader::new("video.mp4");
   let parser = MediaParser::new(reader);
   
   let frame = parser.frame(0, Duration::from_secs(30)).await?;

   println!("Captured frame: {}x{} in {:?} format", frame.width, frame.height, frame.format);
   println!("Frame data: {} bytes", frame.data.len());
   
   match frame.format {
      PixelFormat::Yuv420p => println!("YUV 4:2:0 format detected"),
      PixelFormat::Rgb24 => println!("RGB format detected"),
      _ => println!("Other format: {:?}", frame.format),
   }
   
   Ok(())
}
```

#### Multiple Frames

```rust
use media_parser::{MediaParser, FileStreamReader};
use std::time::Duration;

#[tokio::main]
async fn main() -> media_parser::Result<()> {
   let reader = FileStreamReader::new("video.mp4");
   let parser = MediaParser::new(reader);
   
   let timestamps = vec![
      Duration::from_secs(10),
      Duration::from_secs(30),
      Duration::from_secs(60),
      Duration::from_secs(120),
   ];
   
   let frames = parser.frames(0, &timestamps).await?;
   
   for (i, frame) in frames.iter().enumerate() {
      println!("Frame {}: {}x{} ({:?})", i, frame.width, frame.height, frame.format);
      println!("Data: {} bytes", frame.data.len());
      
      match frame.format {
         PixelFormat::Yuv420p => {
            let y_size = (frame.width * frame.height) as usize;
            let uv_size = y_size / 4;
            println!("Y: {} bytes, U: {} bytes, V: {} bytes", y_size, uv_size, uv_size);
         },
         PixelFormat::Rgb24 => {
            println!("RGB pixels: {}", frame.data.len() / 3);
         },
         _ => {},
      }
   }
   
   println!("Generated {} frames total", frames.len());
   
   Ok(())
}
```

## StreamReader

The parser supports reading local and remote media files via the `StreamReader` trait.
Built-in implementations are provided for local files and HTTP/HTTPS streams, or a custom
implementation can be created for specialized use-cases.

```rust
use async_trait::async_trait;
use std::io::SeekFrom;

/// Trait for reading media streams from various sources
#[async_trait]
pub trait StreamReader: Send + Sync {
   /// Read data into the provided buffer
   async fn read(&mut self, buf: &mut [u8]) -> Result<usize>;
   
   /// Seek to a specific position in the stream
   async fn seek(&mut self, pos: SeekFrom) -> Result<u64>;
   
   /// Get the total size of the stream if known
   async fn size(&self) -> Result<Option<u64>>;
}
```

### Built-in Implementations

#### FileStreamReader

Provides synchronous access to local MP4 files on the filesystem. This implementation is
optimized for local file operations and supports standard file I/O operations.

```rust
/// Local file stream reader
pub struct FileStreamReader {
}

impl FileStreamReader {
   pub fn new<P: AsRef<std::path::Path>>(path: P) -> Self {
      // Implementation...
   }
}

#[async_trait
impl StreamReader for FileStreamReader {
   async fn read(&mut self, buf: &mut [u8]) -> Result<usize> {
      // Read from local file
   }
   
   async fn seek(&mut self, pos: SeekFrom) -> Result<u64> {
      // Seek within local file
   }
   
   async fn size(&self) -> Result<Option<u64>> {
      // Return file size from metadata
   }
}
```

#### HTTPStreamReader

Enables streaming MP4 files over HTTP/HTTPS with support for custom headers and
authentication. Built using [Reqwest](https://docs.rs/reqwest/latest/reqwest/)
for robust network operations.

```rust
use std::collections::HashMap;

/// HTTP/HTTPS remote stream reader
pub struct HTTPStreamReader {
}

impl HTTPStreamReader {
   pub fn new(url: &str) -> Self {
      // ...
   }
   
   pub fn with_headers(url: &str, headers: HashMap<String, String>) -> Self {
      // ...
   }
}

#[async_trait]
impl StreamReader for HTTPStreamReader {
   async fn read(&mut self, buf: &mut [u8]) -> Result<usize> {
      // HTTP range request to read data
   }
   
   async fn seek(&mut self, pos: SeekFrom) -> Result<u64> {
      // Update position for next HTTP range request
   }
   
   async fn size(&self) -> Result<Option<u64>> {
      // HEAD request to get Content-Length
   }
}
```

### Usage Examples

#### Basic FileStreamReader Usage

```rust
use media_parser::{MediaParser, FileStreamReader};

#[tokio::main]
async fn main() -> media_parser::Result<()> {
   let reader = FileStreamReader::new("video.mp4");
   let parser = MediaParser::new(reader);
   
   Ok(())
}
```

#### Basic HTTPStreamReader Usage

```rust
use media_parser::{MediaParser, HTTPStreamReader};
use std::collections::HashMap;

#[tokio::main]
async fn main() -> media_parser::Result<()> {
   // Create HTTP stream reader
   let reader = HTTPStreamReader::new("https://example.com/video.mp4");
   let parser = MediaParser::new(reader);
   
   // Create HTTP stream reader with authentication
   let mut headers = HashMap::new();
   headers.insert("Authorization", "Bearer eyJhbGciOiJIUzI1NiIsInR5cCI6IkpXVCJ9");
   let reader = HTTPStreamReader::with_headers("https://example.com/video.mp4", headers);
   let reader = MediaParser::new(reader);
   
   Ok(())
}
```

## Error Types and Handling

### Error Types

```rust
use std::time::Duration;

/// Error types for media parsing operations
#[derive(Debug, thiserror::Error)]
pub enum MediaParserError {
#[error("I/O error: {0}")]
   Io(#[from] std::io::Error),

   #[error("Invalid MP4 format: {0}")]
   InvalidFormat(String),

   #[error("Corrupted media data at offset {0}")]
   CorruptedData(u64),

   #[error("Track with ID {0} not found")]
   TrackNotFound(u32),

   #[error("Unsupported codec: {0}")]
   UnsupportedCodec(String),

   #[error("Unsupported pixel format: {0:?}")]
   UnsupportedPixelFormat(PixelFormat),

   #[error("Subtitle parsing error: {0}")]
   SubtitleError(String),

   #[error("Metadata key not found: {0}")]
   MetadataKeyNotFound(String),

   #[error("Other error: {0}")]
   Other(String)
}

pub type Result<T> = std::result::Result<T, MediaParserError>;
```

### Error Handling Example

All API methods return `Result<T, MediaParserError>` for comprehensive error handling:

```rust
use media_parser::{MediaParser, FileStreamReader, MediaParserError};

#[tokio::main]
async fn main() {
   let reader = FileStreamReader::new("video.mp4");
   let parser = MediaParser::new(reader);

   // Handle specific errors with detailed matching
   match parser.metadata().await {
      Ok(metadata) => {
         println!("Title: {:?}", metadata.title());
         println!("Duration: {:?}", metadata.duration());
      }
      Err(MediaParserError::InvalidFormat(msg)) => {
         eprintln!("Invalid MP4 format: {}", msg);
      }
      Err(MediaParserError::CorruptedData(offset)) => {
         eprintln!("Media data is corrupted at byte offset {}", offset);
      }
      Err(e) => {
         eprintln!("Unexpected error: {}", e);
      }
   }
}
