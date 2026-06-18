//! Path-based navigation through nested MP4 boxes.

use super::read::read_box;

/// Finds a box by type in a buffer.
///
/// Scans sibling boxes starting from the beginning of `buf` until a box
/// with the specified `target` type is found.
///
/// # Arguments
///
/// * `buf` - Byte buffer starting at a valid box boundary
/// * `target` - 4-byte box type identifier to search for
///
/// # Returns
///
/// Returns `Some((offset, size, payload))` if found:
/// - `offset` - Position of the box within `buf`
/// - `size` - Total box size including header
/// - `payload` - Box content excluding header
///
/// Returns `None` if the box is not found or the buffer is invalid.
pub fn find_box_ref(buf: &[u8], target: [u8; 4]) -> Option<(usize, usize, &[u8])> {
   let mut off = 0usize;

   while let Some(b) = read_box(buf, off) {
      if b.fourcc == target {
         return Some((off, b.total_size, b.payload));
      }
      off += b.total_size;
   }

   None
}

/// Trait for navigating nested MP4 box structures.
pub trait Mp4Nav {
   /// Navigates to a nested box using a path of box type identifiers.
   ///
   /// # Arguments
   ///
   /// * `path` - Sequence of 4-byte box identifiers representing the path
   ///
   /// # Returns
   ///
   /// Returns `Some(payload)` of the final box if all boxes in the path exist.
   /// Returns `None` if any box in the path is not found.
   ///
   /// # Example
   ///
   /// ```no_run
   /// use media_parser::format::mp4::atoms::{Mp4Nav, Mp4Box};
   ///
   /// let moov_data: &[u8] = &[/* moov box payload */];
   /// let path = [Mp4Box::Udta.bytes(), Mp4Box::Meta.bytes()];
   /// if let Some(meta_payload) = moov_data.nav(&path) {
   ///     // Process meta box payload
   /// }
   /// ```
   fn nav(&self, path: &[[u8; 4]]) -> Option<&[u8]>;
}

impl Mp4Nav for [u8] {
   fn nav(&self, path: &[[u8; 4]]) -> Option<&[u8]> {
      path.iter().try_fold(self, |data, &target| {
         find_box_ref(data, target).map(|(_, _, payload)| payload)
      })
   }
}

#[cfg(test)]
mod tests {
   use super::super::types::Mp4Box;
   use super::*;

   #[test]
   fn test_mp4_nav_simple_path() {
      // Create a simple box structure: [ftyp][moov with mvhd inside]
      let mut moov_payload = Vec::new();
      // mvhd box inside moov: size=16 (8 header + 8 payload)
      moov_payload.extend_from_slice(&16u32.to_be_bytes()); // size
      moov_payload.extend_from_slice(b"mvhd");
      moov_payload.extend_from_slice(&[0u8; 8]); // payload (16 - 8 = 8)

      let mut data = Vec::new();
      // ftyp box: size=24 (8 header + 16 payload)
      data.extend_from_slice(&24u32.to_be_bytes()); // size
      data.extend_from_slice(b"ftyp");
      data.extend_from_slice(&[0u8; 16]); // payload (24 - 8 = 16)

      // moov box: size includes header (8) + payload
      let moov_size = 8 + moov_payload.len();
      data.extend_from_slice(&(moov_size as u32).to_be_bytes()); // size
      data.extend_from_slice(b"moov");
      data.extend_from_slice(&moov_payload);

      // Test navigating to moov
      let moov_path = [Mp4Box::Moov.bytes()];
      let moov = data.nav(&moov_path);
      assert!(moov.is_some());

      if let Some(moov_data) = moov {
         // Test navigating to mvhd inside moov
         let mvhd_path = [Mp4Box::Mvhd.bytes()];
         let mvhd = moov_data.nav(&mvhd_path);
         assert!(mvhd.is_some());
      }
   }

   #[test]
   fn test_mp4_nav_nested_path() {
      // Create nested structure: moov -> udta -> meta -> ilst
      let mut data = Vec::new();

      // Build from inside out: ilst
      let mut ilst = Vec::new();
      ilst.extend_from_slice(&8u32.to_be_bytes());
      ilst.extend_from_slice(b"ilst");

      // meta containing ilst
      let mut meta = Vec::new();
      meta.extend_from_slice(&(8 + ilst.len() as u32).to_be_bytes());
      meta.extend_from_slice(b"meta");
      meta.extend_from_slice(&ilst);

      // udta containing meta
      let mut udta = Vec::new();
      udta.extend_from_slice(&(8 + meta.len() as u32).to_be_bytes());
      udta.extend_from_slice(b"udta");
      udta.extend_from_slice(&meta);

      // moov containing udta
      data.extend_from_slice(&(8 + udta.len() as u32).to_be_bytes());
      data.extend_from_slice(b"moov");
      data.extend_from_slice(&udta);

      // Test nested navigation
      let path = [
         Mp4Box::Moov.bytes(),
         Mp4Box::Udta.bytes(),
         Mp4Box::Meta.bytes(),
         Mp4Box::Ilst.bytes(),
      ];
      let ilst_data = data.nav(&path);
      assert!(ilst_data.is_some());
   }

   #[test]
   fn test_mp4_nav_not_found() {
      // Use valid box structure but without moov
      let mut data = Vec::new();
      // ftyp box
      data.extend_from_slice(&16u32.to_be_bytes()); // size = header (8) + payload (8)
      data.extend_from_slice(b"ftyp");
      data.extend_from_slice(&[0u8; 8]);

      let path = [Mp4Box::Moov.bytes()];
      let result = data.nav(&path);
      assert_eq!(result, None);
   }
}
