use crate::{
    Error, Result,
    config::{Fit, Image},
};
use image::{DynamicImage, ImageFormat, ImageReader, Rgba, RgbaImage, imageops};
use std::io::{Cursor, Read};

#[derive(Debug)]
pub struct Prepared {
    pub label: String,
    pub width: u32,
    pub height: u32,
    pub bgra: Vec<u8>,
}

pub fn read_bounded(reader: impl Read, max: u64) -> Result<Vec<u8>> {
    let mut bytes = Vec::new();
    reader.take(max + 1).read_to_end(&mut bytes)?;
    if bytes.len() as u64 > max {
        return Err(Error::Limit);
    }
    Ok(bytes)
}

pub fn decode(bytes: &[u8], config: &Image, label: String) -> Result<Prepared> {
    let format = image::guess_format(bytes)?;
    if !matches!(format, ImageFormat::Png | ImageFormat::Jpeg) {
        return Err(Error::Format);
    }
    let reader = ImageReader::with_format(Cursor::new(bytes), format);
    let (w, h) = reader.into_dimensions()?;
    if w == 0 || h == 0 || u64::from(w) * u64::from(h) > config.max_source_pixels {
        return Err(Error::Limit);
    }
    let mut reader = ImageReader::with_format(Cursor::new(bytes), format);
    let mut limits = image::Limits::default();
    limits.max_image_width = Some(16384);
    limits.max_image_height = Some(16384);
    limits.max_alloc = Some(384 * 1024 * 1024);
    reader.limits(limits);
    let source = reader.decode()?;
    let resized = match config.fit {
        Fit::Cover => {
            source.resize_to_fill(config.width, config.height, imageops::FilterType::Triangle)
        }
        Fit::Contain => source.resize(config.width, config.height, imageops::FilterType::Triangle),
    }
    .into_rgba8();
    let mut canvas = RgbaImage::from_pixel(config.width, config.height, Rgba([0, 0, 0, 255]));
    imageops::overlay(
        &mut canvas,
        &resized,
        i64::from((config.width - resized.width()) / 2),
        i64::from((config.height - resized.height()) / 2),
    );
    let mut bgra = canvas.into_raw();
    for p in bgra.as_chunks_mut::<4>().0 {
        p.swap(0, 2);
        p[3] = 255;
    }
    Ok(Prepared {
        label,
        width: config.width,
        height: config.height,
        bgra,
    })
}

pub fn write_preview(prepared: &Prepared, path: &std::path::Path) -> Result<()> {
    let mut rgba = prepared.bgra.clone();
    for p in rgba.as_chunks_mut::<4>().0 {
        p.swap(0, 2);
    }
    let image = RgbaImage::from_raw(prepared.width, prepared.height, rgba).ok_or(Error::Limit)?;
    DynamicImage::ImageRgba8(image).save(path)?;
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn bounded_read_and_decode_limits() {
        assert!(matches!(read_bounded(&b"12345"[..], 4), Err(Error::Limit)));
        let src = DynamicImage::ImageRgba8(RgbaImage::from_pixel(8, 4, Rgba([255, 0, 0, 255])));
        let mut bytes = Cursor::new(Vec::new());
        src.write_to(&mut bytes, ImageFormat::Png).unwrap();
        let mut cfg = Image {
            width: 4,
            height: 4,
            fit: Fit::Contain,
            max_download_mb: 1,
            max_source_pixels: 32,
        };
        let out = decode(bytes.get_ref(), &cfg, "fixture".into()).unwrap();
        assert_eq!(&out.bgra[..4], &[0, 0, 0, 255]);
        assert_eq!(&out.bgra[16..20], &[0, 0, 255, 255]);
        cfg.fit = Fit::Cover;
        let out = decode(bytes.get_ref(), &cfg, "fixture".into()).unwrap();
        assert_eq!(&out.bgra[..4], &[0, 0, 255, 255]);
        cfg.max_source_pixels = 31;
        assert!(matches!(
            decode(bytes.get_ref(), &cfg, "".into()),
            Err(Error::Limit)
        ));
        assert!(decode(b"not a picture", &cfg, "".into()).is_err());
    }
}
