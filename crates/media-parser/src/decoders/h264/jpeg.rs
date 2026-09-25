use super::color::GopColor;
use super::convert::yuv_to_rgb;
use super::frame::PlanarYuv;
use super::{DecodeError, DecodedImage, JpegQuality, ThumbnailSize};

pub(crate) fn yuv_to_jpeg(
   yuv: &PlanarYuv<'_>,
   rgb: &mut Vec<u8>,
   quality: JpegQuality,
   size: ThumbnailSize,
   color: GopColor,
) -> Result<DecodedImage, DecodeError> {
   let (width, height) = yuv_to_rgb(yuv, rgb, size, color)?;
   let data = encode_jpeg(rgb, width as usize, height as usize, quality)?;
   Ok(DecodedImage {
      width,
      height,
      data,
   })
}

fn encode_jpeg(
   rgb: &mut [u8],
   width: usize,
   height: usize,
   quality: JpegQuality,
) -> Result<Vec<u8>, DecodeError> {
   let width_u16 = u16::try_from(width)
      .map_err(|_| DecodeError::Convert("frame width exceeds JPEG limits".into()))?;
   let height_u16 = u16::try_from(height)
      .map_err(|_| DecodeError::Convert("frame height exceeds JPEG limits".into()))?;
   if width == 0
      || height == 0
      || rgb.len() != super::convert::rgb_buffer_len(u32::from(width_u16), u32::from(height_u16))?
   {
      return Err(DecodeError::Convert(
         "invalid JPEG RGB dimensions or buffer length".into(),
      ));
   }
   crate::encoders::jpeg::encode_jpeg(rgb, width, height, quality)
}

#[cfg(test)]
mod tests {
   use super::*;

   #[test]
   fn encoder_rejects_invalid_rgb() {
      for (width, height, len) in [
         (0, 1, 0),
         (1, 0, 0),
         (1, 1, 2),
         (1, 1, 4),
         (usize::MAX, 1, 3),
      ] {
         assert!(matches!(
            encode_jpeg(&mut vec![0; len], width, height, JpegQuality::default()),
            Err(DecodeError::Convert(_))
         ));
      }
   }
}
