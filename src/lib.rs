//! # Tauri Plugin Media Parser
//!
//! A Tauri plugin for parsing media files, providing access to metadata,
//! tracks, frames, and subtitles from both local files and remote HTTP streams.
//!
//! ## Features
//!
//! - Extract metadata (title, artist, album, duration, etc.) from media formats
//! - Support for local file paths and remote URLs (HTTP/HTTPS)
//! - Custom HTTP headers for authenticated requests
//! - Async streaming with efficient range requests
//!
//! ## Usage
//!
//! ### Rust (Plugin Registration)
//!
//! Register the plugin in your Tauri application:
//!
//! ```rust,ignore,no_run
//! fn main() {
//!     tauri::Builder::default()
//!         .plugin(tauri_plugin_media_parser::init())
//!         .run(tauri::generate_context!())
//!         .expect("error while running tauri application");
//! }
//! ```
//!
//! ### TypeScript (Frontend)
//!
//! ```typescript,ignore
//! import { getMetadata } from '@silvermine/tauri-plugin-media-parser';
//!
//! // Local file
//! const metadata = await getMetadata('/path/to/video.mp4');
//!
//! // Remote URL
//! const metadata = await getMetadata('https://example.com/video.mp4');
//!
//! // With authentication headers
//! const metadata = await getMetadata('https://example.com/video.mp4', {
//!     headers: { 'Authorization': 'Bearer token123' }
//! });
//!
//! console.log(`Duration: ${metadata.duration / metadata.timescale} seconds`);
//! ```

use std::collections::HashMap;
use tauri::{Manager, Runtime, plugin::TauriPlugin};

#[cfg(any(target_os = "android", test))]
mod android_jpeg;
mod commands;
mod envelope;
mod error;
mod session_cache;
mod source;
mod subtitle_command;

pub use error::{Error, Result};
pub use media_parser;

#[cfg(target_os = "ios")]
unsafe extern "C" {
   fn tauri_plugin_media_parser_link_videotoolbox();
}

/// Initializes the media-parser plugin.
///
/// Call this function in your Tauri application's builder to register
/// the plugin and enable its commands.
///
/// # Example
///
/// ```rust,ignore,no_run
/// fn main() {
///     tauri::Builder::default()
///         .plugin(tauri_plugin_media_parser::init())
///         .run(tauri::generate_context!())
///         .expect("error while running tauri application");
/// }
/// ```
pub fn init<R: Runtime>() -> TauriPlugin<R> {
   Builder::new().build()
}

/// Configures HTTP defaults on all platforms.
/// Per-call headers override defaults, except `Host` when defaults apply to the requested URL.
#[derive(Default)]
pub struct Builder {
   user_agent: Option<String>,
   headers: HashMap<String, String>,
   origins: Option<Vec<String>>,
}

impl Builder {
   /// Creates a builder without HTTP defaults.
   pub fn new() -> Self {
      Self::default()
   }

   /// Sets the user agent, taking precedence over `User-Agent` in default headers.
   pub fn user_agent(mut self, value: impl Into<String>) -> Self {
      self.user_agent = Some(value.into());
      self
   }

   /// Adds default headers. Later values replace earlier ones regardless of name casing.
   /// Without `default_headers_origins`, defaults go to any URL requested by the frontend.
   pub fn default_headers<K: Into<String>, V: Into<String>>(
      mut self,
      headers: impl IntoIterator<Item = (K, V)>,
   ) -> Self {
      self.headers.extend(
         headers
            .into_iter()
            .map(|(name, value)| (name.into().to_ascii_lowercase(), value.into())),
      );
      self
   }

   /// Restricts all HTTP defaults, including the user agent, to these HTTP(S) origins.
   /// Calls accumulate origins; an initially empty list allows none. Unset, defaults are global.
   /// Other destinations still receive per-call headers, but none of these defaults.
   /// Invalid URLs or non-HTTP(S) schemes fail plugin initialization.
   pub fn default_headers_origins(
      mut self,
      origins: impl IntoIterator<Item = impl Into<String>>,
   ) -> Self {
      self
         .origins
         .get_or_insert_with(Vec::new)
         .extend(origins.into_iter().map(Into::into));
      self
   }

   fn into_headers(mut self) -> HashMap<String, String> {
      if let Some(user_agent) = self.user_agent {
         self.headers.insert("user-agent".into(), user_agent);
      }
      self.headers
   }

   fn into_default_headers(mut self) -> Result<source::DefaultHeaders> {
      let origins = self.origins.take();
      let headers = self.into_headers();
      for (name, value) in &headers {
         tauri::http::HeaderName::from_bytes(name.as_bytes())
            .map_err(|error| Error::Custom(format!("Invalid default header name: {error}")))?;
         tauri::http::HeaderValue::from_str(value)
            .map_err(|error| Error::Custom(format!("Invalid default header value: {error}")))?;
      }
      source::DefaultHeaders::new(headers, origins)
   }

   /// Builds the plugin. Invalid HTTP headers fail plugin initialization.
   pub fn build<R: Runtime>(self) -> TauriPlugin<R> {
      #[cfg(target_os = "ios")]
      unsafe {
         tauri_plugin_media_parser_link_videotoolbox();
      }
      let plugin = tauri::plugin::Builder::new("media-parser");
      #[cfg(target_os = "android")]
      let plugin = plugin.on_webview_ready(android_jpeg::on_webview_ready);
      plugin
         .setup(move |app, _api| {
            app.manage(self.into_default_headers()?);
            #[cfg(native_h264_backend)]
            app.manage(commands::ThumbnailSessions::default());
            #[cfg(not(native_h264_backend))]
            app.manage(commands::ThumbnailSessions);
            app.manage(subtitle_command::SubtitleSessions::default());
            Ok(())
         })
         .invoke_handler(tauri::generate_handler![
            commands::get_metadata,
            commands::get_tracks,
            commands::get_cover,
            commands::get_thumbnails,
            subtitle_command::get_subtitles
         ])
         .build()
   }
}

#[cfg(test)]
mod tests {
   use super::*;
   use rstest::rstest;
   use tauri::test::{mock_builder, mock_context, noop_assets};

   #[rstest]
   #[case::configured_origin(
      "https://example.com/file",
      source::MergedHeaders {
         headers: Some(HashMap::from([
            ("user-agent".into(), "app/1.0".into()),
            ("authorization".into(), "Bearer test".into()),
         ])),
         force_same_origin: true,
      },
   )]
   #[case::other_origin("https://other.example/file", source::MergedHeaders::default())]
   fn plugin_setup_registers_configured_default_headers(
      #[case] url: &str,
      #[case] expected: source::MergedHeaders,
   ) {
      let app = mock_builder()
         .plugin(
            Builder::new()
               .user_agent("app/1.0")
               .default_headers([("Authorization", "Bearer test")])
               .default_headers_origins(["https://example.com"])
               .build(),
         )
         .build(mock_context(noop_assets()))
         .expect("valid defaults should allow plugin initialization");

      let defaults: tauri::State<'_, source::DefaultHeaders> = app.state();
      assert_eq!(defaults.merge(url, None).unwrap(), expected);
   }

   #[test]
   fn plugin_setup_rejects_invalid_default_header() {
      let result = mock_builder()
         .plugin(Builder::new().default_headers([("X-Test", "a\nb")]).build())
         .build(mock_context(noop_assets()));

      match result {
         Err(tauri::Error::PluginInitialization(name, reason)) => {
            assert_eq!(name, "media-parser");
            assert!(reason.contains("Invalid default header value"), "{reason}");
         }
         Err(error) => panic!("unexpected initialization error: {error}"),
         Ok(_) => panic!("invalid defaults should reject plugin initialization"),
      }
   }

   #[rstest]
   #[case::first_origin("https://first.example/file", true)]
   #[case::second_origin("https://second.example/file", true)]
   #[case::unlisted_origin("https://third.example/file", false)]
   fn default_header_origins_accumulate_and_limit_where_defaults_are_sent(
      #[case] source: &str,
      #[case] sends_defaults: bool,
   ) {
      let defaults = Builder::new()
         .user_agent("app/1.0")
         .default_headers_origins(Vec::<String>::new())
         .default_headers_origins(["https://first.example"])
         .default_headers_origins(["https://second.example"])
         .into_default_headers()
         .unwrap();
      let expected =
         sends_defaults.then(|| HashMap::from([("user-agent".into(), "app/1.0".into())]));
      assert_eq!(defaults.merge(source, None).unwrap().headers, expected);
   }

   #[test]
   fn empty_default_header_origins_send_no_defaults() {
      let defaults = Builder::new()
         .user_agent("app/1.0")
         .default_headers([("Authorization", "Bearer test")])
         .default_headers_origins(Vec::<String>::new())
         .into_default_headers()
         .unwrap();
      assert_eq!(
         defaults
            .merge("https://example.com/file", None)
            .unwrap()
            .headers,
         None
      );
   }

   #[rstest]
   #[case::invalid_user_agent_value(Builder::new().user_agent("a\nb"))]
   #[case::invalid_header_name(Builder::new().default_headers([("invalid header", "value")]))]
   #[case::invalid_header_value(Builder::new().default_headers([("X-Test", "a\nb")]))]
   fn invalid_http_defaults_fail_default_header_composition(#[case] builder: Builder) {
      assert!(matches!(
         builder.into_default_headers(),
         Err(Error::Custom(_))
      ));
   }

   #[test]
   fn later_default_headers_replace_earlier_values_ignoring_name_casing() {
      let headers = Builder::new()
         .default_headers([("X-Test", "first")])
         .default_headers([("x-test", "second")])
         .into_headers();

      assert_eq!(headers, HashMap::from([("x-test".into(), "second".into())]));
   }

   #[rstest]
   #[case::user_agent_set_first(
      Builder::new()
         .user_agent("my-app/1.0")
         .default_headers([("User-Agent", "other/1.0")]),
   )]
   #[case::user_agent_set_last(
      Builder::new()
         .default_headers([("User-Agent", "other/1.0")])
         .user_agent("my-app/1.0"),
   )]
   fn user_agent_overrides_default_header_in_either_setter_order(#[case] builder: Builder) {
      assert_eq!(
         builder
            .into_default_headers()
            .unwrap()
            .merge("https://example.com/file", None)
            .unwrap()
            .headers,
         Some(HashMap::from([("user-agent".into(), "my-app/1.0".into())])),
      );
   }
}
