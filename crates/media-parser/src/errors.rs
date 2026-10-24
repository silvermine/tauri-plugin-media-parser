use crate::types::PixelFormat;

/// Error types for media parsing operations
#[derive(Debug, thiserror::Error)]
#[non_exhaustive]
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

   #[error("Decoder error: {0}")]
   Decode(String),

   #[error("Output limit exceeded: {0}")]
   OutputLimit(String),

   #[error("Resource limit reached: {0}")]
   ResourceLimit(String),

   #[error("Unsupported pixel format: {0:?}")]
   UnsupportedPixelFormat(PixelFormat),

   #[error("Subtitle parsing error: {0}")]
   SubtitleError(String),

   #[error("Metadata key not found: {0}")]
   MetadataKeyNotFound(String),

   /// Error occurred in a blocking task (spawn_blocking)
   #[error("Blocking task error: {0}")]
   BlockingTask(String),

   /// HTTP request failed
   #[error("HTTP request failed: {0}")]
   HttpRequest(String),

   /// HTTP response had an error status code
   #[error("HTTP error status: {0}")]
   HttpStatus(u16),

   /// Content-Length header missing or invalid
   #[error("Content-Length header missing or invalid")]
   ContentLengthMissing,

   /// Content length not available
   #[error("Content length not available")]
   ContentLengthUnavailable,

   /// Other error (fallback for cases not covered above)
   #[error("Other error: {0}")]
   Other(String),
}

pub type Result<T> = std::result::Result<T, MediaParserError>;

#[cfg(test)]
mod tests {
   use super::MediaParserError;
   use rstest::rstest;

   #[rstest]
   #[case::decode_separately_from_unsupported_codec(
      MediaParserError::Decode("native decoder stopped".to_string()),
      "Decoder error: native decoder stopped"
   )]
   #[case::output_limit_as_user_adjustable_limit(
      MediaParserError::OutputLimit("thumbnail payload is too large".to_string()),
      "Output limit exceeded: thumbnail payload is too large"
   )]
   #[case::resource_limit_separately_from_output_limit(
      MediaParserError::ResourceLimit("allocation failed".to_string()),
      "Resource limit reached: allocation failed"
   )]
   fn renders_error_category_messages(#[case] error: MediaParserError, #[case] expected: &str) {
      assert_eq!(error.to_string(), expected);
   }
}
