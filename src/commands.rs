use std::collections::HashMap;
use tauri::command;
use url::Url;

use media_parser::{FileStreamReader, HttpStreamReader, MediaParser, Metadata};

use crate::Result;

/// Extract metadata from a media file (local path or URL).
///
/// # Arguments
/// * `source` - Absolute path to a local file or URL of a remote media file
/// * `headers` - Optional custom HTTP headers (only used for URLs, e.g., for authentication)
///
/// # Returns
/// Metadata containing duration, timescale, and tags (title, artist, etc.)
#[command]
pub(crate) async fn get_metadata(
   source: String,
   headers: Option<HashMap<String, String>>,
) -> Result<Metadata> {
   let is_http_url = Url::parse(&source)
      .map(|url| matches!(url.scheme(), "http" | "https"))
      .unwrap_or(false);

   if is_http_url {
      let reader = match headers {
         Some(h) => HttpStreamReader::with_headers(&source, h).await?,
         None => HttpStreamReader::new(&source).await?,
      };
      let parser = MediaParser::new(reader);
      parser.metadata().await.map_err(Into::into)
   } else {
      let reader = FileStreamReader::new(&source)?;
      let parser = MediaParser::new(reader);
      parser.metadata().await.map_err(Into::into)
   }
}
