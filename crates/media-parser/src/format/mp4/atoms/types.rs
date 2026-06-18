//! MP4 box type definitions.

/// Known MP4 box types used for metadata extraction.
///
/// # Example
///
/// ```no_run
/// use media_parser::format::mp4::atoms::Mp4Box;
///
/// let moov_id = Mp4Box::Moov.bytes();
/// assert_eq!(moov_id, *b"moov");
/// ```
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Mp4Box {
   /// Movie box containing media metadata and track information.
   Moov,
   /// Movie header box with timescale and duration.
   Mvhd,
   /// User data box.
   Udta,
   /// Metadata box.
   Meta,
   /// Item list box containing metadata key-value pairs.
   Ilst,
   /// Data box containing the value for a metadata item.
   Data,
}

impl Mp4Box {
   /// Returns the 4-byte identifier for this box type.
   pub const fn bytes(self) -> [u8; 4] {
      match self {
         Self::Moov => *b"moov",
         Self::Mvhd => *b"mvhd",
         Self::Udta => *b"udta",
         Self::Meta => *b"meta",
         Self::Ilst => *b"ilst",
         Self::Data => *b"data",
      }
   }
}

#[cfg(test)]
mod tests {
   use super::*;

   #[test]
   fn test_mp4_box_bytes() {
      assert_eq!(Mp4Box::Moov.bytes(), *b"moov");
      assert_eq!(Mp4Box::Mvhd.bytes(), *b"mvhd");
      assert_eq!(Mp4Box::Udta.bytes(), *b"udta");
      assert_eq!(Mp4Box::Meta.bytes(), *b"meta");
      assert_eq!(Mp4Box::Ilst.bytes(), *b"ilst");
      assert_eq!(Mp4Box::Data.bytes(), *b"data");
   }
}
