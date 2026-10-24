//! # Text Encoding/Decoding Utilities
//!
//! Functions for decoding text.
//!
//! ## Supported Encodings
//!
//! - ISO-8859-1 (Latin-1)
//! - UTF-8
//! - UTF-16 with BOM (LE/BE)
//! - UTF-16BE without BOM

/// Text encoding types.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum TextEncoding {
   /// ISO-8859-1 (Latin-1)
   Latin1,
   /// UTF-8
   Utf8,
   /// UTF-16 with BOM
   Utf16Bom,
   /// UTF-16 Big-Endian without BOM
   Utf16Be,
   /// UTF-16 Little-Endian without BOM
   Utf16Le,
}

/// Decodes text from bytes using the specified encoding.
pub fn decode_text(data: &[u8], encoding: TextEncoding) -> Option<String> {
   match encoding {
      TextEncoding::Latin1 => decode_latin1(data),
      TextEncoding::Utf8 => decode_utf8(data),
      TextEncoding::Utf16Bom => decode_utf16_with_bom(data),
      TextEncoding::Utf16Be => decode_utf16_be(data),
      TextEncoding::Utf16Le => decode_utf16_le(data),
   }
}

/// Decodes ISO-8859-1 (Latin-1) text.
pub fn decode_latin1(data: &[u8]) -> Option<String> {
   if data.is_empty() {
      return None;
   }
   Some(data.iter().map(|&b| b as char).collect())
}

/// Decodes UTF-8 text.
pub fn decode_utf8(data: &[u8]) -> Option<String> {
   if data.is_empty() {
      return None;
   }
   Some(String::from_utf8_lossy(data).to_string())
}

/// Decodes UTF-16 text with BOM detection.
///
/// Detects byte order from BOM (0xFEFF for BE, 0xFFFE for LE).
/// If no BOM is present, assumes big-endian.
pub fn decode_utf16_with_bom(data: &[u8]) -> Option<String> {
   if data.len() < 2 {
      return None;
   }

   let (is_le, start) = match (data[0], data[1]) {
      (0xFF, 0xFE) => (true, 2),  // Little-endian BOM
      (0xFE, 0xFF) => (false, 2), // Big-endian BOM
      _ => (false, 0),            // No BOM, assume big-endian
   };

   let bytes = &data[start..];
   decode_utf16_bytes(bytes, is_le)
}

/// Decodes UTF-16 Big-Endian text (no BOM).
pub fn decode_utf16_be(data: &[u8]) -> Option<String> {
   decode_utf16_bytes(data, false)
}

/// Decodes UTF-16 Little-Endian text (no BOM).
pub fn decode_utf16_le(data: &[u8]) -> Option<String> {
   decode_utf16_bytes(data, true)
}

/// Internal: decodes UTF-16 bytes with specified endianness.
fn decode_utf16_bytes(data: &[u8], little_endian: bool) -> Option<String> {
   if data.is_empty() || !data.len().is_multiple_of(2) {
      return None;
   }

   let units: Vec<u16> = data
      .chunks_exact(2)
      .map(|chunk| {
         if little_endian {
            u16::from_le_bytes([chunk[0], chunk[1]])
         } else {
            u16::from_be_bytes([chunk[0], chunk[1]])
         }
      })
      .collect();

   String::from_utf16(&units).ok()
}

/// Trims null terminators and whitespace from a string.
pub fn trim_null_and_whitespace(s: &str) -> Option<String> {
   let trimmed = s.trim_matches(char::from(0)).trim().to_string();
   if trimmed.is_empty() {
      None
   } else {
      Some(trimmed)
   }
}

#[cfg(test)]
mod tests {
   use super::*;
   use rstest::rstest;

   #[rstest]
   #[case::ascii(b"Hello", Some("Hello".to_string()))]
   #[case::empty(&[], None)]
   fn test_decode_latin1(#[case] data: &[u8], #[case] expected: Option<String>) {
      assert_eq!(decode_latin1(data), expected);
   }

   #[rstest]
   #[case::ascii(b"Hello", Some("Hello".to_string()))]
   #[case::cjk("日本語".as_bytes(), Some("日本語".to_string()))]
   #[case::empty(&[], None)]
   fn test_decode_utf8(#[case] data: &[u8], #[case] expected: Option<String>) {
      assert_eq!(decode_utf8(data), expected);
   }

   #[rstest]
   // BOM=FE FF (BE), "Hi"
   #[case::be_bom(&[0xFE, 0xFF, 0x00, 0x48, 0x00, 0x69])]
   // BOM=FF FE (LE), "Hi"
   #[case::le_bom(&[0xFF, 0xFE, 0x48, 0x00, 0x69, 0x00])]
   // No BOM, assumes BE, "Hi"
   #[case::no_bom(&[0x00, 0x48, 0x00, 0x69])]
   fn test_decode_utf16_with_bom(#[case] data: &[u8]) {
      assert_eq!(decode_utf16_with_bom(data), Some("Hi".to_string()));
   }

   #[test]
   fn test_decode_utf16_be() {
      let data = [0x00, 0x48, 0x00, 0x69]; // "Hi" in BE
      assert_eq!(decode_utf16_be(&data), Some("Hi".to_string()));
   }

   #[test]
   fn test_decode_utf16_le() {
      let data = [0x48, 0x00, 0x69, 0x00]; // "Hi" in LE
      assert_eq!(decode_utf16_le(&data), Some("Hi".to_string()));
   }

   #[test]
   fn test_decode_utf16_empty() {
      assert_eq!(decode_utf16_be(&[]), None);
      assert_eq!(decode_utf16_le(&[]), None);
   }

   #[test]
   fn test_decode_utf16_odd_length() {
      assert_eq!(decode_utf16_be(&[0x00]), None);
      assert_eq!(decode_utf16_le(&[0x00, 0x48, 0x00]), None);
   }

   #[rstest]
   #[case::trailing_nulls("Hello\0\0", Some("Hello".to_string()))]
   #[case::surrounding_nulls("\0\0Hello\0\0", Some("Hello".to_string()))]
   #[case::surrounding_spaces("  Hello  ", Some("Hello".to_string()))]
   #[case::only_nulls("\0\0\0", None)]
   #[case::only_spaces("   ", None)]
   #[case::empty("", None)]
   fn test_trim_null_and_whitespace(#[case] input: &str, #[case] expected: Option<String>) {
      assert_eq!(trim_null_and_whitespace(input), expected);
   }

   #[rstest]
   #[case::latin1(TextEncoding::Latin1)]
   #[case::utf8(TextEncoding::Utf8)]
   fn test_decode_text_dispatch(#[case] encoding: TextEncoding) {
      assert_eq!(decode_text(b"Hello", encoding), Some("Hello".to_string()));
   }
}
