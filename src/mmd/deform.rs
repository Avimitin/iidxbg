use super::*;
use glam::{Mat4, Vec4};

/// D3D9 XYZ RHW / diffuse / UV. Fully owned, prepared on the animation worker.
#[repr(C)]
#[derive(Clone, Copy, Debug)]
pub struct Vertex {
    pub position: [f32; 4],
    pub color: u32,
    pub uv: [f32; 2],
}

pub(super) fn validate(p: &format::PmxParsedModel) -> Result<()> {
    let g = &p.geometry;
    let count = g.positions.len() / 3;
    let bones = p.skeleton.bones.len();
    if count == 0
        || count > 100_000
        || bones == 0
        || bones > 512
        || p.materials.is_empty()
        || p.materials.len() > 256
        || p.morphs.len() > 512
        || g.indices.len() > 600_000
        || g.indices.is_empty()
        || !g.indices.len().is_multiple_of(3)
        || g.normals.len() != count * 3
        || g.uvs.len() != count * 2
        || g.skin_indices.len() != count * 4
        || g.skin_weights.len() != count * 4
        || g.indices.iter().any(|i| *i as usize >= count)
        || g.positions
            .iter()
            .chain(&g.normals)
            .chain(&g.uvs)
            .chain(&g.skin_weights)
            .any(|x| !x.is_finite() || x.abs() > 1_000_000.)
    {
        return Err(Error::Mmd(
            "invalid or excessive PMX mesh dimensions/data".into(),
        ));
    }
    for (indices, weights) in g
        .skin_indices
        .as_chunks::<4>()
        .0
        .iter()
        .zip(g.skin_weights.as_chunks::<4>().0)
    {
        if weights.iter().any(|w| *w < 0. || *w > 1.)
            || (weights.iter().sum::<f32>() - 1.).abs() > 0.01
            || indices
                .iter()
                .zip(weights)
                .any(|(i, w)| *w > 0. && *i as usize >= bones)
        {
            return Err(Error::Mmd("invalid PMX skin weights".into()));
        }
    }
    let mut total = 0;
    for group in &g.material_groups {
        if group.start != total
            || group.material_index >= p.materials.len()
            || !group.count.is_multiple_of(3)
        {
            return Err(Error::Mmd("invalid material ranges".into()));
        }
        total = total.checked_add(group.count).ok_or(Error::Limit)?;
    }
    if total != g.indices.len() {
        return Err(Error::Mmd("material ranges do not cover mesh".into()));
    }
    for b in &p.skeleton.bones {
        if b.position.iter().any(|v| !v.is_finite())
            || b.ik.as_ref().is_some_and(|ik| ik.links.len() > 64)
        {
            return Err(Error::Limit);
        }
    }
    for m in &p.materials {
        if m.diffuse.iter().chain(&m.ambient).any(|x| !x.is_finite()) {
            return Err(Error::Mmd("nonfinite material".into()));
        }
    }
    let mut offsets = 0usize;
    for m in &p.morphs {
        offsets += m.vertex_offsets.len()
            + m.uv_offsets.len()
            + m.material_offsets.len()
            + m.group_offsets.len()
            + m.bone_offsets.len();
        if m.vertex_offsets
            .iter()
            .any(|o| o.vertex_index as usize >= count || o.position.iter().any(|x| !x.is_finite()))
            || m.uv_offsets
                .iter()
                .any(|o| o.vertex_index as usize >= count || o.uv.iter().any(|x| !x.is_finite()))
            || m.material_offsets.iter().any(|o| {
                o.material_index < -1
                    || o.material_index >= p.materials.len() as i32
                    || o.diffuse
                        .iter()
                        .chain(&o.texture_factor)
                        .any(|x| !x.is_finite())
            })
        {
            return Err(Error::Mmd("invalid morph offsets".into()));
        }
    }
    if offsets > 2_000_000 {
        return Err(Error::Limit);
    }
    if g.sdef.enabled.len() != count
        || g.sdef.c.len() != count * 3
        || g.sdef.r0.len() != count * 3
        || g.sdef.r1.len() != count * 3
        || g.sdef
            .c
            .iter()
            .chain(&g.sdef.r0)
            .chain(&g.sdef.r1)
            .any(|v| !v.is_finite())
    {
        return Err(Error::Mmd("invalid SDEF data".into()));
    }
    for (i, enabled) in g.sdef.enabled.iter().enumerate() {
        if *enabled > 0.
            && g.skin_indices[i * 4..i * 4 + 2]
                .iter()
                .any(|b| *b as usize >= bones)
        {
            return Err(Error::Mmd("invalid SDEF bone indices".into()));
        }
    }
    Ok(())
}

fn linear(
    position: Vec3,
    normal: Vec3,
    indices: &[u32],
    weights: &[f32],
    matrices: &[Mat4],
) -> (Vec3, Vec3) {
    let (mut p, mut n) = (Vec3::ZERO, Vec3::ZERO);
    for (&i, &w) in indices.iter().zip(weights) {
        if w <= 0. {
            continue;
        }
        let m = matrices[i as usize];
        p += m.transform_point3(position) * w;
        n += m.transform_vector3(normal) * w;
    }
    (p, n.normalize_or_zero())
}
fn dual(
    position: Vec3,
    normal: Vec3,
    indices: &[u32],
    weights: &[f32],
    matrices: &[Mat4],
) -> (Vec3, Vec3) {
    let mut real = Vec4::ZERO;
    let mut dual = Vec4::ZERO;
    let mut reference = None;
    for (&i, &w) in indices.iter().zip(weights) {
        if w <= 0. {
            continue;
        }
        let (_, r, t) = matrices[i as usize].to_scale_rotation_translation();
        let q = Vec4::from_array(r.to_array());
        let first = *reference.get_or_insert(q);
        let w = if first.dot(q) < 0. { -w } else { w };
        real += q * w;
        dual += Vec4::from_array((Quat::from_xyzw(t.x, t.y, t.z, 0.) * r).to_array()) * (0.5 * w);
    }
    let len = real.length();
    if len < 1e-6 {
        return (position, normal);
    }
    real /= len;
    dual /= len;
    dual -= real * real.dot(dual);
    let r = Quat::from_array(real.to_array());
    let t = Quat::from_array(dual.to_array()) * r.conjugate();
    (r * position + Vec3::new(t.x, t.y, t.z) * 2., r * normal)
}

pub(super) fn frame(model: &Model, runtime: &RuntimeInstance, serial: u64) -> Result<Frame> {
    let p = &model.parsed;
    let g = &p.geometry;
    let mut positions: Vec<Vec3> = g
        .positions
        .as_chunks::<3>()
        .0
        .iter()
        .copied()
        .map(Vec3::from_array)
        .collect();
    let mut uvs: Vec<[f32; 2]> = g.uvs.as_chunks::<2>().0.to_vec();
    let mut materials: Vec<_> = p.materials.iter().map(|m| m.diffuse).collect();
    for (m, &weight) in p.morphs.iter().zip(runtime.morph_weights()) {
        if weight.abs() < 1e-6 {
            continue;
        }
        for o in &m.vertex_offsets {
            positions[o.vertex_index as usize] += Vec3::from_array(o.position) * weight;
        }
        for o in &m.uv_offsets {
            let uv = &mut uvs[o.vertex_index as usize];
            uv[0] += o.uv[0] * weight;
            uv[1] += o.uv[1] * weight;
        }
        for o in &m.material_offsets {
            for (i, color) in materials.iter_mut().enumerate() {
                if o.material_index != -1 && o.material_index != i as i32 {
                    continue;
                }
                for (c, v) in color.iter_mut().zip(o.diffuse) {
                    if o.operation == "multiply" {
                        *c *= 1. + (v - 1.) * weight;
                    } else {
                        *c += v * weight;
                    }
                }
            }
        }
    }
    let matrices = runtime.pose().skinning_matrices();
    let rotation = Quat::from_rotation_y(model.settings.yaw.to_radians());
    let scale = model.settings.height as f32 * model.settings.scale / model.extent;
    let light = Vec3::new(-0.3, 0.6, -0.7).normalize();
    let mut vertices = Vec::with_capacity(positions.len());
    for (i, position) in positions.into_iter().enumerate() {
        let normal = Vec3::from_slice(&g.normals[i * 3..i * 3 + 3]);
        let indices = &g.skin_indices[i * 4..i * 4 + 4];
        let weights = &g.skin_weights[i * 4..i * 4 + 4];
        let (position, normal) = if g.sdef.enabled[i] > 0. {
            let a = matrices[indices[0] as usize];
            let b = matrices[indices[1] as usize];
            let c = Vec3::from_slice(&g.sdef.c[i * 3..i * 3 + 3]);
            let r0 = Vec3::from_slice(&g.sdef.r0[i * 3..i * 3 + 3]);
            let r1 = Vec3::from_slice(&g.sdef.r1[i * 3..i * 3 + 3]);
            let rw = r0 * weights[0] + r1 * weights[1];
            let qa = a.to_scale_rotation_translation().1;
            let qb = b.to_scale_rotation_translation().1;
            let q = qa.slerp(qb, weights[1]);
            (
                q * (position - c)
                    + a.transform_point3(c + r0 - rw) * weights[0]
                    + b.transform_point3(c + r1 - rw) * weights[1],
                q * normal,
            )
        } else if g.qdef.enabled.get(i).is_some_and(|v| *v > 0.) {
            dual(position, normal, indices, weights, matrices)
        } else {
            linear(position, normal, indices, weights, matrices)
        };
        let position = rotation * (position - model.center);
        let normal = rotation * normal;
        let brightness = (0.72 + 0.28 * normal.dot(light).max(0.)).clamp(0., 1.);
        let shade = (brightness * 255.).round() as u32;
        let screen = [
            model.settings.width as f32 * 0.5 + position.x * scale - 0.5,
            model.settings.height as f32 * 0.5 - position.y * scale - 0.5,
            (0.5 + position.z / (model.extent * 8.)).clamp(0.001, 0.999),
            1.,
        ];
        if screen.iter().chain(&uvs[i]).any(|v| !v.is_finite()) {
            return Err(Error::Mmd("nonfinite animated vertex".into()));
        }
        vertices.push(Vertex {
            position: screen,
            color: 0xff000000 | (shade << 16) | (shade << 8) | shade,
            uv: uvs[i],
        });
    }
    Ok(Frame {
        vertices,
        materials,
        serial,
    })
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn skinning_translation_and_dual_quaternion_preserve_normals() {
        let m = [
            Mat4::from_translation(Vec3::new(4., 0., 0.)),
            Mat4::from_translation(Vec3::new(0., 2., 0.)),
        ];
        for f in [linear, dual] {
            let (p, n) = f(Vec3::X, Vec3::Y, &[0, 1, 0, 0], &[0.5, 0.5, 0., 0.], &m);
            assert!((p - Vec3::new(3., 1., 0.)).length() < 1e-5);
            assert!((n - Vec3::Y).length() < 1e-5);
        }
    }
}
