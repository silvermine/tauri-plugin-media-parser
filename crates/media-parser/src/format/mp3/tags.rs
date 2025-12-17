//! ID3v2 frame tag definitions lookup.
//!
//! Provides a sorted array of frame definitions for binary search lookup,
//! mirroring the pattern used in mp4/atoms/tags.rs.

/// Frame definition with 4-char ID and human-readable name.
#[derive(Debug, Clone, Copy)]
struct FrameDef {
   id: [u8; 4],
   name: &'static str,
}

/// Sorted array of known ID3v2 frame IDs.
///
/// Must remain sorted by id for binary search to work correctly.
/// Reference: https://id3.org/id3v2.3.0#Declared_ID3v2_frames
const FRAME_DEFS: &[FrameDef] = &[
   FrameDef {
      id: *b"COMM",
      name: "Comment",
   },
   FrameDef {
      id: *b"TALB",
      name: "Album",
   },
   FrameDef {
      id: *b"TBPM",
      name: "BPM",
   },
   FrameDef {
      id: *b"TCOM",
      name: "Composer",
   },
   FrameDef {
      id: *b"TCON",
      name: "Genre",
   },
   FrameDef {
      id: *b"TCOP",
      name: "Copyright",
   },
   FrameDef {
      id: *b"TDRC",
      name: "Year",
   },
   FrameDef {
      id: *b"TENC",
      name: "Encoder",
   },
   FrameDef {
      id: *b"TIT2",
      name: "Title",
   },
   FrameDef {
      id: *b"TLAN",
      name: "Language",
   },
   FrameDef {
      id: *b"TLEN",
      name: "Length",
   },
   FrameDef {
      id: *b"TPE1",
      name: "Artist",
   },
   FrameDef {
      id: *b"TPE2",
      name: "Album Artist",
   },
   FrameDef {
      id: *b"TPOS",
      name: "Disc Number",
   },
   FrameDef {
      id: *b"TPUB",
      name: "Publisher",
   },
   FrameDef {
      id: *b"TRCK",
      name: "Track Number",
   },
   FrameDef {
      id: *b"TYER",
      name: "Year",
   },
];

/// Returns the human-readable name for a frame ID - O(log n).
///
/// Uses binary search on a sorted array of known frames.
///
/// # Example
///
/// ```no_run
/// use media_parser::format::mp3::tags::frame_name;
///
/// assert_eq!(frame_name(*b"TIT2"), "Title");
/// assert_eq!(frame_name(*b"TPE1"), "Artist");
/// assert_eq!(frame_name(*b"XXXX"), "Unknown");
/// ```
#[inline]
pub fn frame_name(id: [u8; 4]) -> &'static str {
   FRAME_DEFS
      .binary_search_by_key(&id, |def| def.id)
      .map(|i| FRAME_DEFS[i].name)
      .unwrap_or("Unknown")
}

/// Converts frame ID bytes to a string key.
#[inline]
pub fn frame_id_to_key(id: [u8; 4]) -> String {
   String::from_utf8_lossy(&id).into_owned()
}

#[cfg(test)]
mod tests {
   use super::*;

   #[test]
   fn test_frame_defs_sorted() {
      // Verify array is sorted for binary search
      for i in 1..FRAME_DEFS.len() {
         assert!(
            FRAME_DEFS[i - 1].id < FRAME_DEFS[i].id,
            "FRAME_DEFS not sorted at index {}: {:?} >= {:?}",
            i,
            FRAME_DEFS[i - 1].id,
            FRAME_DEFS[i].id
         );
      }
   }

   #[test]
   fn test_frame_name_known() {
      assert_eq!(frame_name(*b"TIT2"), "Title");
      assert_eq!(frame_name(*b"TPE1"), "Artist");
      assert_eq!(frame_name(*b"TALB"), "Album");
      assert_eq!(frame_name(*b"TRCK"), "Track Number");
      assert_eq!(frame_name(*b"TPOS"), "Disc Number");
      assert_eq!(frame_name(*b"COMM"), "Comment");
   }

   #[test]
   fn test_frame_name_unknown() {
      assert_eq!(frame_name(*b"XXXX"), "Unknown");
      assert_eq!(frame_name(*b"\x00\x00\x00\x00"), "Unknown");
   }

   #[test]
   fn test_frame_id_to_key() {
      assert_eq!(frame_id_to_key(*b"TIT2"), "TIT2");
      assert_eq!(frame_id_to_key(*b"TPE1"), "TPE1");
   }
}
