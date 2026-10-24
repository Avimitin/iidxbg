use crate::{
    Error, Result,
    config::{Fit, Image},
};
use image::{DynamicImage, ImageFormat, ImageReader, RgbaImage, imageops};
use std::io::{Cursor, Read};

#[derive(Clone, Debug)]
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
    decode_with_canvas(bytes, config, label, false)
}

/// A character cutout keeps alpha and removes empty margins. The native sprite
/// supplies placement; user artwork never needs a pre-positioned screen canvas.
pub fn decode_character(bytes: &[u8], config: &Image, label: String) -> Result<Prepared> {
    let mut config = config.clone();
    config.width = 1920;
    config.height = 1080;
    decode_with_canvas(bytes, &config, label, true)
}

fn decode_with_canvas(
    bytes: &[u8],
    config: &Image,
    label: String,
    character: bool,
) -> Result<Prepared> {
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
    let canvas = if character {
        let rgba = source.into_rgba8();
        let [left, top, right, bottom] =
            character_bounds(&rgba).ok_or(Error::Config("character image is fully transparent"))?;
        let cropped = imageops::crop_imm(&rgba, left, top, right - left, bottom - top).to_image();
        DynamicImage::ImageRgba8(cropped)
            .resize(config.width, config.height, imageops::FilterType::Triangle)
            .into_rgba8()
    } else if config.resource {
        source
            .resize(
                config.width.min(w),
                config.height.min(h),
                imageops::FilterType::Triangle,
            )
            .into_rgba8()
    } else {
        fit_canvas(&source, config.width, config.height, config.fit)
    };
    let (width, height) = canvas.dimensions();
    let mut bgra = canvas.into_raw();
    for p in bgra.as_chunks_mut::<4>().0 {
        p.swap(0, 2);
        if !character && !config.resource {
            p[3] = 255;
        }
    }
    Ok(Prepared {
        label,
        width,
        height,
        bgra,
    })
}

fn character_bounds(image: &RgbaImage) -> Option<[u32; 4]> {
    let mut bounds = [image.width(), image.height(), 0, 0];
    for (x, y, pixel) in image.enumerate_pixels() {
        if pixel[3] != 0 {
            bounds[0] = bounds[0].min(x);
            bounds[1] = bounds[1].min(y);
            bounds[2] = bounds[2].max(x + 1);
            bounds[3] = bounds[3].max(y + 1);
        }
    }
    (bounds[2] > bounds[0] && bounds[3] > bounds[1]).then_some(bounds)
}

/// Worker-only compatibility canvas for older native root-sprite adapters.
pub fn character_canvas(image: Prepared) -> Prepared {
    let source =
        RgbaImage::from_raw(image.width, image.height, image.bgra).expect("prepared pixels");
    let mut canvas = RgbaImage::new(1920, 1080);
    imageops::overlay(
        &mut canvas,
        &source,
        i64::from((1920 - source.width()) / 2),
        i64::from((1080 - source.height()) / 2),
    );
    Prepared {
        label: image.label,
        width: 1920,
        height: 1080,
        bgra: canvas.into_raw(),
    }
}

/// Worker-only derivative; channel order does not affect the resize filter.
pub fn resize(prepared: &Prepared, width: u32, height: u32, fit: Fit) -> Result<Prepared> {
    let source = RgbaImage::from_raw(prepared.width, prepared.height, prepared.bgra.clone())
        .ok_or(Error::Limit)?;
    let source = DynamicImage::ImageRgba8(source);
    let canvas = fit_canvas(&source, width, height, fit);
    Ok(Prepared {
        label: prepared.label.clone(),
        width,
        height,
        bgra: canvas.into_raw(),
    })
}

// Blur a small stretched copy, then upscale it. This bounds blur cost even on a
// 4K output canvas. Only the backdrop is blurred; the centered foreground is sharp.
fn fit_canvas(source: &DynamicImage, width: u32, height: u32, fit: Fit) -> RgbaImage {
    if fit == Fit::Cover {
        return source
            .resize_to_fill(width, height, imageops::FilterType::Triangle)
            .into_rgba8();
    }
    let foreground = source
        .resize(width, height, imageops::FilterType::Triangle)
        .into_rgba8();
    let mut canvas = if foreground.width() == width && foreground.height() == height {
        return foreground;
    } else {
        source
            .resize_exact(
                width.min(256),
                height.min(256),
                imageops::FilterType::Triangle,
            )
            .blur(16.0)
            .resize_exact(width, height, imageops::FilterType::Triangle)
            .into_rgba8()
    };
    // PNG transparency must not leave transparent borders in the final texture.
    for pixel in canvas.pixels_mut() {
        pixel[3] = 255;
    }
    imageops::overlay(
        &mut canvas,
        &foreground,
        i64::from((width - foreground.width()) / 2),
        i64::from((height - foreground.height()) / 2),
    );
    canvas
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
    use image::Rgba;
    #[test]
    fn character_bounds_ignore_empty_padding_and_keep_faint_edges() {
        let mut image = RgbaImage::new(100, 100);
        assert_eq!(character_bounds(&image), None);
        image.put_pixel(37, 12, Rgba([255, 20, 30, 1]));
        image.put_pixel(50, 80, Rgba([255, 20, 30, 255]));
        assert_eq!(character_bounds(&image), Some([37, 12, 51, 81]));
    }
    #[test]
    fn contain_blurs_only_the_margin_and_keeps_foreground_sharp() {
        let source = RgbaImage::from_fn(64, 32, |x, _| {
            if x % 2 == 0 {
                Rgba([255, 0, 0, 255])
            } else {
                Rgba([0, 0, 255, 255])
            }
        });
        let canvas = fit_canvas(
            &DynamicImage::ImageRgba8(source.clone()),
            64,
            64,
            Fit::Contain,
        );
        for x in 0..64 {
            assert_eq!(canvas.get_pixel(x, 16), source.get_pixel(x, 0));
        }
        let margin = canvas.get_pixel(32, 0);
        assert!(margin[0] > 30 && margin[0] < 225);
        assert!(margin[2] > 30 && margin[2] < 225);
        assert_eq!(margin[3], 255);
    }
    #[test]
    fn surface_derivatives_preserve_bgra_and_fit_without_mutating_source() {
        let source = Prepared {
            label: "source".into(),
            width: 4,
            height: 2,
            bgra: [17, 45, 200, 255].repeat(8),
        };
        let cover = resize(&source, 2, 4, Fit::Cover).unwrap();
        assert_eq!(cover.bgra, [17, 45, 200, 255].repeat(8));
        let contain = resize(&source, 2, 4, Fit::Contain).unwrap();
        assert_eq!(&contain.bgra[..8], &[17, 45, 200, 255].repeat(2));
        assert_eq!(&contain.bgra[8..16], &[17, 45, 200, 255].repeat(2));
        assert_eq!(source.bgra, [17, 45, 200, 255].repeat(8));
        assert_eq!(cover.label, source.label);
    }

    #[test]
    fn bounded_read_and_decode_limits() {
        assert!(matches!(read_bounded(&b"12345"[..], 4), Err(Error::Limit)));
        let src = DynamicImage::ImageRgba8(RgbaImage::from_pixel(8, 4, Rgba([255, 0, 0, 255])));
        let mut bytes = Cursor::new(Vec::new());
        src.write_to(&mut bytes, ImageFormat::Png).unwrap();
        let mut cfg = Image {
            resource: false,
            width: 4,
            height: 4,
            fit: Fit::Contain,
            max_download_mb: 1,
            max_source_pixels: 32,
        };
        let out = decode(bytes.get_ref(), &cfg, "fixture".into()).unwrap();
        assert_eq!(&out.bgra[..4], &[0, 0, 255, 255]);
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

    #[test]
    fn resource_decode_preserves_alpha_and_aspect_without_upscaling() {
        let source = DynamicImage::ImageRgba8(RgbaImage::from_pixel(8, 4, Rgba([210, 30, 10, 64])));
        let mut bytes = Cursor::new(Vec::new());
        source.write_to(&mut bytes, ImageFormat::Png).unwrap();
        let config = Image {
            resource: true,
            width: 4,
            height: 4,
            fit: Fit::Cover,
            max_download_mb: 1,
            max_source_pixels: 32,
        };
        let pixels = decode(bytes.get_ref(), &config, "resource".into()).unwrap();
        assert_eq!((pixels.width, pixels.height), (4, 2));
        assert!(
            pixels
                .bgra
                .as_chunks::<4>()
                .0
                .iter()
                .all(|p| *p == [10, 30, 210, 64])
        );
        let larger = Image {
            width: 128,
            height: 128,
            ..config
        };
        let pixels = decode(bytes.get_ref(), &larger, "resource".into()).unwrap();
        assert_eq!((pixels.width, pixels.height), (8, 4));
    }
}
