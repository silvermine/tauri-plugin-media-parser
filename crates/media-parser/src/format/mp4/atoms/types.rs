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
   use rstest::rstest;

   #[rstest]
   #[case::moov(Mp4Box::Moov, *b"moov")]
   #[case::mvhd(Mp4Box::Mvhd, *b"mvhd")]
   #[case::udta(Mp4Box::Udta, *b"udta")]
   #[case::meta(Mp4Box::Meta, *b"meta")]
   #[case::ilst(Mp4Box::Ilst, *b"ilst")]
   #[case::data(Mp4Box::Data, *b"data")]
   fn test_mp4_box_bytes(#[case] mp4_box: Mp4Box, #[case] expected: [u8; 4]) {
      assert_eq!(mp4_box.bytes(), expected);
   }
}
