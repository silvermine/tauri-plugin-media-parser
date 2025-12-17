//! Iterator for traversing sibling MP4 boxes.

use super::read::read_box;

/// Iterator over sibling MP4 boxes in a byte buffer.
///
/// Each iteration yields a tuple of `(box_type, payload)` where:
/// - `box_type` is the 4-byte box identifier
/// - `payload` is the box content (excluding the header)
///
/// Supports both standard (8-byte) and extended (16-byte) headers.
///
/// Iteration stops when:
/// - The buffer is exhausted
/// - A box with invalid size is encountered
pub struct Mp4BoxIter<'a> {
   data: &'a [u8],
   offset: usize,
}

impl<'a> Iterator for Mp4BoxIter<'a> {
   type Item = ([u8; 4], &'a [u8]);

   fn next(&mut self) -> Option<Self::Item> {
      let b = read_box(self.data, self.offset)?;
      self.offset += b.total_size;
      Some((b.fourcc, b.payload))
   }
}

impl<'a> Mp4BoxIter<'a> {
   /// Finds the first box with the specified type - O(k) where k = boxes skipped.
   ///
   /// Each skip is O(1) - only reads header, doesn't process payload.
   /// Returns the payload of the found box, or `None` if not found.
   ///
   /// Supports both standard and extended size boxes.
   ///
   /// # Example
   ///
   /// ```no_run
   /// use media_parser::format::mp4::atoms::iter_boxes;
   ///
   /// let data: &[u8] = &[/* MP4 box data */];
   /// if let Some(moov_payload) = iter_boxes(data).find_box(*b"moov") {
   ///     println!("Found moov: {} bytes", moov_payload.len());
   /// }
   /// ```
   #[inline]
   pub fn find_box(mut self, target: [u8; 4]) -> Option<&'a [u8]> {
      loop {
         let b = read_box(self.data, self.offset)?;
         self.offset += b.total_size;
         if b.fourcc == target {
            return Some(b.payload);
         }
      }
   }
}

/// Creates an iterator over sibling MP4 boxes in a buffer.
///
/// # Example
///
/// ```no_run
/// use media_parser::format::mp4::atoms::iter_boxes;
///
/// let data: &[u8] = &[/* MP4 box data */];
/// for (box_type, payload) in iter_boxes(data) {
///     println!("Box: {:?}, size: {}", box_type, payload.len());
/// }
/// ```
pub fn iter_boxes(data: &[u8]) -> Mp4BoxIter<'_> {
   Mp4BoxIter { data, offset: 0 }
}

#[cfg(test)]
mod tests {
   use super::*;

   #[test]
   fn test_iter_boxes() {
      // Create multiple boxes: ftyp, moov, mdat
      let mut data = Vec::new();

      // ftyp box: size=16 (8 header + 8 payload)
      data.extend_from_slice(&16u32.to_be_bytes());
      data.extend_from_slice(b"ftyp");
      data.extend_from_slice(&[0u8; 8]); // payload

      // moov box: size=16 (8 header + 8 payload)
      data.extend_from_slice(&16u32.to_be_bytes());
      data.extend_from_slice(b"moov");
      data.extend_from_slice(&[0u8; 8]); // payload

      // mdat box: size=12 (8 header + 4 payload)
      data.extend_from_slice(&12u32.to_be_bytes());
      data.extend_from_slice(b"mdat");
      data.extend_from_slice(&[0u8; 4]); // payload

      let boxes: Vec<_> = iter_boxes(&data).collect();
      assert_eq!(boxes.len(), 3);
      assert_eq!(boxes[0].0, *b"ftyp");
      assert_eq!(boxes[1].0, *b"moov");
      assert_eq!(boxes[2].0, *b"mdat");
   }

   #[test]
   fn test_iter_boxes_empty() {
      let data = vec![0u8; 0];
      let boxes: Vec<_> = iter_boxes(&data).collect();
      assert_eq!(boxes.len(), 0);
   }

   #[test]
   fn test_iter_boxes_invalid_size() {
      // Box with size smaller than header
      let mut data = Vec::new();
      data.extend_from_slice(&4u32.to_be_bytes()); // size < 8
      data.extend_from_slice(b"test");

      let boxes: Vec<_> = iter_boxes(&data).collect();
      assert_eq!(boxes.len(), 0);
   }

   #[test]
   fn test_find_box() {
      let mut data = Vec::new();

      // ftyp box
      data.extend_from_slice(&16u32.to_be_bytes());
      data.extend_from_slice(b"ftyp");
      data.extend_from_slice(&[1u8; 8]);

      // moov box
      data.extend_from_slice(&16u32.to_be_bytes());
      data.extend_from_slice(b"moov");
      data.extend_from_slice(&[2u8; 8]);

      // mdat box
      data.extend_from_slice(&12u32.to_be_bytes());
      data.extend_from_slice(b"mdat");
      data.extend_from_slice(&[3u8; 4]);

      // Find moov (skips ftyp)
      let moov = iter_boxes(&data).find_box(*b"moov");
      assert!(moov.is_some());
      assert_eq!(moov.unwrap(), &[2u8; 8]);

      // Find ftyp (first box)
      let ftyp = iter_boxes(&data).find_box(*b"ftyp");
      assert!(ftyp.is_some());
      assert_eq!(ftyp.unwrap(), &[1u8; 8]);

      // Find non-existent box
      let none = iter_boxes(&data).find_box(*b"trak");
      assert!(none.is_none());
   }
}
