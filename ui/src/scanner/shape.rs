//! Offline GE2D shape-fill mapping, matching inventory-geometry.py.
use super::{Result, xml::slice};
pub fn preview(b: &[u8], render: bool) -> Result<Option<(u32, u32, image::RgbaImage)>> {
    let little = match slice(b, 0, 4)? {
        b"D2EG" => true,
        b"GE2D" => false,
        _ => return Err("unknown GE2D magic".into()),
    };
    let u16at = |p| -> Result<usize> {
        let v = slice(b, p, 2)?.try_into().unwrap();
        Ok(if little {
            u16::from_le_bytes(v)
        } else {
            u16::from_be_bytes(v)
        } as usize)
    };
    let u32at = |p| -> Result<u32> {
        let v = slice(b, p, 4)?.try_into().unwrap();
        Ok(if little {
            u32::from_le_bytes(v)
        } else {
            u32::from_be_bytes(v)
        })
    };
    if u32at(12)? as usize != b.len() {
        return Err("GE2D size mismatch".into());
    }
    let vertices = u16at(20)?;
    let draws = u16at(28)?;
    let vo = u32at(32)? as usize;
    let ro = u32at(48)? as usize;
    slice(b, vo, vertices * 8)?;
    slice(b, ro, draws * 16)?;
    if vertices == 0 || draws == 0 || (0..draws).any(|i| b[ro + i * 16 + 1] != 9) {
        return Ok(None);
    }
    let mut points = Vec::with_capacity(vertices);
    let (mut x0, mut y0, mut x1, mut y1) = (
        f32::INFINITY,
        f32::INFINITY,
        f32::NEG_INFINITY,
        f32::NEG_INFINITY,
    );
    for i in 0..vertices {
        let x = f32::from_bits(u32at(vo + i * 8)?);
        let y = f32::from_bits(u32at(vo + i * 8 + 4)?);
        if !x.is_finite() || !y.is_finite() {
            return Err("non-finite geometry".into());
        }
        x0 = x0.min(x);
        y0 = y0.min(y);
        x1 = x1.max(x);
        y1 = y1.max(y);
        points.push((x, y));
    }
    let w = (x1 - x0).ceil();
    let h = (y1 - y0).ceil();
    if !(1.0..=16384.0).contains(&w) || !(1.0..=16384.0).contains(&h) {
        return Err("invalid shape bounds".into());
    }
    let scale = (256.0 / w).min(256.0 / h);
    if !render {
        return Ok(Some((w as u32, h as u32, image::RgbaImage::new(0, 0))));
    }
    let mut image = image::RgbaImage::new(
        (w * scale).round().max(1.) as u32,
        (h * scale).round().max(1.) as u32,
    );
    for p in &mut points {
        p.0 = (p.0 - x0) * scale;
        p.1 = (p.1 - y0) * scale;
    }
    let edge = |a: (f32, f32), b: (f32, f32), p: (f32, f32)| {
        (b.0 - a.0) * (p.1 - a.1) - (b.1 - a.1) * (p.0 - a.0)
    };
    let mut triangle_count = 0;
    for i in 0..draws {
        let r = ro + i * 16;
        let count = u16at(r + 4)?;
        let indices = u32at(r + 12)? as usize;
        slice(b, indices, count * 2)?;
        triangle_count += count / 3;
        if triangle_count > 100_000 {
            return Err("shape complexity limit".into());
        }
        let color = image::Rgba(slice(b, r + 8, 4)?.try_into().unwrap());
        for t in (0..count.saturating_sub(2)).step_by(3) {
            let a = *points
                .get(u16at(indices + t * 2)?)
                .ok_or("shape index out of bounds")?;
            let c = *points
                .get(u16at(indices + (t + 1) * 2)?)
                .ok_or("shape index out of bounds")?;
            let d = *points
                .get(u16at(indices + (t + 2) * 2)?)
                .ok_or("shape index out of bounds")?;
            if edge(a, c, d).abs() < f32::EPSILON {
                continue;
            }
            let minx = a.0.min(c.0).min(d.0).floor().max(0.) as u32;
            let miny = a.1.min(c.1).min(d.1).floor().max(0.) as u32;
            let maxx = (a.0.max(c.0).max(d.0).ceil() as u32).min(image.width());
            let maxy = (a.1.max(c.1).max(d.1).ceil() as u32).min(image.height());
            for y in miny..maxy {
                for x in minx..maxx {
                    let p = (x as f32 + 0.5, y as f32 + 0.5);
                    let e = [edge(a, c, p), edge(c, d, p), edge(d, a, p)];
                    if e.iter().all(|v| *v >= 0.) || e.iter().all(|v| *v <= 0.) {
                        image.put_pixel(x, y, color);
                    }
                }
            }
        }
    }
    Ok(Some((w as u32, h as u32, image)))
}
