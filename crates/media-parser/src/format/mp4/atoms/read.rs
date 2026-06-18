//! Core box reading primitive.
//!
//! This ensures consistent handling of:
//! - Standard 8-byte headers
//! - Extended 16-byte headers (size32 == 1)
//! - Bounds validation

use crate::helpers::{read_u32_be, read_u64_be};

/// Box header info (without payload reference).
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct BoxHeader {
   /// 4-byte box type identifier (fourcc)
   pub fourcc: [u8; 4],
   /// Header length (8 for standard, 16 for extended)
   pub header_len: usize,
   /// Total box size including header
   pub total_size: usize,
}

/// Result of reading a box at a specific offset.
///
/// Representation of a parsed box,
/// containing all information needed for further processing.
#[derive(Debug, Clone, Copy)]
pub struct BoxRead<'a> {
   /// 4-byte box type identifier (fourcc)
   pub fourcc: [u8; 4],
   /// Header length (8 for standard, 16 for extended)
   pub header_len: usize,
   /// Total box size including header
   pub total_size: usize,
   /// Box payload (content after header)
   pub payload: &'a [u8],
}

/// Reads only the box header.
///
/// Unlike `read_box`, this only needs the header bytes (8 or 16),
/// not the entire box content. Useful for discovering box positions
/// when the full box may extend beyond the buffer.
#[inline]
pub fn read_box_header(data: &[u8], offset: usize) -> Option<BoxHeader> {
   if offset + 8 > data.len() {
      return None;
   }

   let size32 = read_u32_be(data, offset)?;

   let (header_len, total_size) = if size32 == 1 {
      if offset + 16 > data.len() {
         return None;
      }
      (16, read_u64_be(data, offset + 8)? as usize)
   } else if size32 == 0 {
      return None;
   } else {
      (8, size32 as usize)
   };

   if total_size < header_len {
      return None;
   }

   Some(BoxHeader {
      fourcc: [
         data[offset + 4],
         data[offset + 5],
         data[offset + 6],
         data[offset + 7],
      ],
      header_len,
      total_size,
   })
}

/// Reads a box at any offset.
///
/// Handles both standard (8-byte) and extended (16-byte) headers.
///
/// # Arguments
///
/// * `data` - Byte buffer containing box data
/// * `offset` - Offset where the box starts
///
/// # Returns
///
/// Returns `Some(BoxRead)` if a valid box exists at the offset.
/// Returns `None` if:
/// - Insufficient data for header
/// - Invalid size (< header length or exceeds buffer)
/// - Size is 0 (extends to EOF - not supported in slice context)
///
/// # Example
///
/// ```no_run
/// use media_parser::format::mp4::atoms::read_box;
///
/// let data = [
///     0, 0, 0, 16,           // size = 16
///     b'm', b'o', b'o', b'v', // fourcc = "moov"
///     1, 2, 3, 4, 5, 6, 7, 8  // payload (8 bytes)
/// ];
///
/// let b = read_box(&data, 0).unwrap();
/// assert_eq!(b.fourcc, *b"moov");
/// assert_eq!(b.header_len, 8);
/// assert_eq!(b.total_size, 16);
/// assert_eq!(b.payload, &[1, 2, 3, 4, 5, 6, 7, 8]);
/// ```
#[inline]
pub fn read_box(data: &[u8], offset: usize) -> Option<BoxRead<'_>> {
   // Need at least 8 bytes for standard header
   if offset + 8 > data.len() {
      return None;
   }

   let size32 = read_u32_be(data, offset)?;

   let (header_len, total_size) = if size32 == 1 {
      // Extended size: 64-bit size follows fourcc
      if offset + 16 > data.len() {
         return None;
      }
      (16, read_u64_be(data, offset + 8)? as usize)
   } else if size32 == 0 {
      // Size 0 means box extends to EOF - not supported in slice context
      return None;
   } else {
      (8, size32 as usize)
   };

   // Validate size
   if total_size < header_len || offset + total_size > data.len() {
      return None;
   }

   Some(BoxRead {
      fourcc: [
         data[offset + 4],
         data[offset + 5],
         data[offset + 6],
         data[offset + 7],
      ],
      header_len,
      total_size,
      payload: &data[offset + header_len..offset + total_size],
   })
}

#[cfg(test)]
mod tests {
   use super::*;

   fn make_box(fourcc: &[u8; 4], payload: &[u8]) -> Vec<u8> {
      let size = 8 + payload.len();
      let mut data = Vec::with_capacity(size);
      data.extend_from_slice(&(size as u32).to_be_bytes());
      data.extend_from_slice(fourcc);
      data.extend_from_slice(payload);
      data
   }

   fn make_extended_box(fourcc: &[u8; 4], payload: &[u8]) -> Vec<u8> {
      let size = 16 + payload.len();
      let mut data = Vec::with_capacity(size);
      data.extend_from_slice(&1u32.to_be_bytes()); // size32 = 1
      data.extend_from_slice(fourcc);
      data.extend_from_slice(&(size as u64).to_be_bytes());
      data.extend_from_slice(payload);
      data
   }

   #[test]
   fn test_read_box_standard() {
      let data = make_box(b"moov", &[1, 2, 3, 4]);

      let b = read_box(&data, 0).unwrap();
      assert_eq!(b.fourcc, *b"moov");
      assert_eq!(b.header_len, 8);
      assert_eq!(b.total_size, 12);
      assert_eq!(b.payload, &[1, 2, 3, 4]);
   }

   #[test]
   fn test_read_box_extended() {
      let data = make_extended_box(b"mdat", &[5, 6, 7, 8, 9, 10]);

      let b = read_box(&data, 0).unwrap();
      assert_eq!(b.fourcc, *b"mdat");
      assert_eq!(b.header_len, 16);
      assert_eq!(b.total_size, 22);
      assert_eq!(b.payload, &[5, 6, 7, 8, 9, 10]);
   }

   #[test]
   fn test_read_box_with_offset() {
      let mut data = make_box(b"ftyp", &[0; 8]);
      data.extend(make_box(b"moov", &[1, 2, 3, 4]));

      // Read second box at offset 16
      let b = read_box(&data, 16).unwrap();
      assert_eq!(b.fourcc, *b"moov");
      assert_eq!(b.payload, &[1, 2, 3, 4]);
   }

   #[test]
   fn test_read_box_insufficient_data() {
      let data = [0, 0, 0, 16, b'm', b'o', b'o', b'v']; // Claims 16, only 8

      assert!(read_box(&data, 0).is_none());
   }

   #[test]
   fn test_read_box_zero_size() {
      let data = [0, 0, 0, 0, b'm', b'o', b'o', b'v'];

      assert!(read_box(&data, 0).is_none());
   }

   #[test]
   fn test_read_box_size_too_small() {
      let data = [0, 0, 0, 4, b't', b'e', b's', b't']; // size 4 < header 8

      assert!(read_box(&data, 0).is_none());
   }

   #[test]
   fn test_read_box_out_of_bounds() {
      let data = make_box(b"test", &[1, 2, 3, 4]);

      assert!(read_box(&data, 100).is_none());
   }

   #[test]
   fn test_read_box_empty_payload() {
      let data = [0, 0, 0, 8, b't', b'e', b's', b't']; // size 8, no payload

      let b = read_box(&data, 0).unwrap();
      assert_eq!(b.fourcc, *b"test");
      assert_eq!(b.payload.len(), 0);
   }
}
