use super::{Axis, GizmoMode};

pub(crate) type Geo = (Vec<[f32; 3]>, Vec<[f32; 3]>, Vec<u16>);

pub(crate) fn merge(a: Geo, b: Geo) -> Geo {
    let (mut pa, mut na, mut ia) = a;
    let (pb, nb, ib) = b;
    let base = pa.len() as u16;
    pa.extend(pb);
    na.extend(nb);
    ia.extend(ib.into_iter().map(|i| i + base));
    (pa, na, ia)
}

pub(crate) fn translate_geo(g: Geo, t: [f32; 3]) -> Geo {
    let (p, n, i) = g;
    (
        p.into_iter().map(|q| [q[0] + t[0], q[1] + t[1], q[2] + t[2]]).collect(),
        n,
        i,
    )
}

pub(crate) fn apply_rot(g: Geo, f: fn([f32; 3]) -> [f32; 3]) -> Geo {
    let (p, n, i) = g;
    (p.into_iter().map(f).collect(), n.into_iter().map(f).collect(), i)
}

fn cylinder(radius: f32, height: f32, segments: usize) -> Geo {
    let mut positions = Vec::new();
    let mut normals = Vec::new();
    let mut indices = Vec::new();
    for i in 0..=segments {
        let a = i as f32 / segments as f32 * std::f32::consts::TAU;
        let (s, c) = (a.sin(), a.cos());
        let n = [c, s, 0.0];
        positions.push([radius * c, radius * s, 0.0]);
        normals.push(n);
        positions.push([radius * c, radius * s, height]);
        normals.push(n);
    }
    for i in 0..segments {
        let i0 = (i * 2) as u16;
        let (i1, i2, i3) = (i0 + 1, i0 + 2, i0 + 3);
        indices.extend_from_slice(&[i0, i2, i1, i1, i2, i3]);
    }
    (positions, normals, indices)
}

fn cone(radius: f32, height: f32, base_z: f32, segments: usize) -> Geo {
    let mut positions = Vec::new();
    let mut normals = Vec::new();
    let mut indices = Vec::new();
    let apex = [0.0, 0.0, base_z + height];
    let slope = radius / height.max(1e-4);

    for i in 0..segments {
        let a0 = i as f32 / segments as f32 * std::f32::consts::TAU;
        let a1 = (i + 1) as f32 / segments as f32 * std::f32::consts::TAU;
        let p0 = [radius * a0.cos(), radius * a0.sin(), base_z];
        let p1 = [radius * a1.cos(), radius * a1.sin(), base_z];
        let n0 = super::math::normalize3([a0.cos(), a0.sin(), slope]);
        let n1 = super::math::normalize3([a1.cos(), a1.sin(), slope]);
        let mid = (a0 + a1) * 0.5;
        let napex = super::math::normalize3([mid.cos(), mid.sin(), slope]);
        let base = positions.len() as u16;
        positions.extend_from_slice(&[p0, p1, apex]);
        normals.extend_from_slice(&[n0, n1, napex]);
        indices.extend_from_slice(&[base, base + 1, base + 2]);
    }

    let cap_center_idx = positions.len() as u16;
    positions.push([0.0, 0.0, base_z]);
    normals.push([0.0, 0.0, -1.0]);
    let cap_start = positions.len() as u16;
    for i in 0..segments {
        let a = i as f32 / segments as f32 * std::f32::consts::TAU;
        positions.push([radius * a.cos(), radius * a.sin(), base_z]);
        normals.push([0.0, 0.0, -1.0]);
    }
    for i in 0..segments {
        let a = cap_start + i as u16;
        let b = cap_start + ((i + 1) % segments) as u16;
        indices.extend_from_slice(&[cap_center_idx, b, a]);
    }
    (positions, normals, indices)
}

fn torus(major_r: f32, minor_r: f32, major_seg: usize, minor_seg: usize) -> Geo {
    let mut positions = Vec::new();
    let mut normals = Vec::new();
    let mut indices = Vec::new();
    for i in 0..=major_seg {
        let u = i as f32 / major_seg as f32 * std::f32::consts::TAU;
        let (cu, su) = (u.cos(), u.sin());
        for j in 0..=minor_seg {
            let v = j as f32 / minor_seg as f32 * std::f32::consts::TAU;
            let (cv, sv) = (v.cos(), v.sin());
            let normal = [cu * cv, su * cv, sv];
            let center = [major_r * cu, major_r * su, 0.0];
            positions.push([
                center[0] + minor_r * normal[0],
                center[1] + minor_r * normal[1],
                center[2] + minor_r * normal[2],
            ]);
            normals.push(normal);
        }
    }
    let stride = (minor_seg + 1) as u16;
    for i in 0..major_seg as u16 {
        for j in 0..minor_seg as u16 {
            let a = i * stride + j;
            let (b, c, d) = (a + 1, a + stride, a + stride + 1);
            indices.extend_from_slice(&[a, c, b, b, c, d]);
        }
    }
    (positions, normals, indices)
}

fn cube(half: f32, center: [f32; 3]) -> Geo {
    let h = half;
    let corners = [
        [-h, -h, -h], [h, -h, -h], [h, h, -h], [-h, h, -h],
        [-h, -h, h],  [h, -h, h],  [h, h, h],  [-h, h, h],
    ];
    let faces: [([usize; 4], [f32; 3]); 6] = [
        ([0, 1, 2, 3], [0.0, 0.0, -1.0]),
        ([4, 5, 6, 7], [0.0, 0.0, 1.0]),
        ([0, 1, 5, 4], [0.0, -1.0, 0.0]),
        ([3, 2, 6, 7], [0.0, 1.0, 0.0]),
        ([0, 3, 7, 4], [-1.0, 0.0, 0.0]),
        ([1, 2, 6, 5], [1.0, 0.0, 0.0]),
    ];
    let mut positions = Vec::new();
    let mut normals = Vec::new();
    let mut indices = Vec::new();
    for (idx, n) in faces.iter() {
        let base = positions.len() as u16;
        for &k in idx {
            let p = corners[k];
            positions.push([p[0] + center[0], p[1] + center[1], p[2] + center[2]]);
            normals.push(*n);
        }
        indices.extend_from_slice(&[base, base + 2, base + 1, base, base + 3, base + 2]);
    }
    (positions, normals, indices)
}

fn quad_double_sided_rect(w: f32, h: f32) -> Geo {
    let hw = w * 0.5;
    let hh = h * 0.5;
    let p = [[-hw, -hh, 0.0], [hw, -hh, 0.0], [hw, hh, 0.0], [-hw, hh, 0.0]];
    let mut positions = Vec::new();
    let mut normals = Vec::new();
    let mut indices = Vec::new();

    positions.extend_from_slice(&p);
    normals.extend_from_slice(&[[0.0, 0.0, 1.0]; 4]);
    indices.extend_from_slice(&[0, 1, 2, 0, 2, 3]);

    let base = positions.len() as u16;
    positions.extend_from_slice(&p);
    normals.extend_from_slice(&[[0.0, 0.0, -1.0]; 4]);
    indices.extend_from_slice(&[base, base + 2, base + 1, base, base + 3, base + 2]);

    (positions, normals, indices)
}

fn quad_double_sided(size: f32) -> Geo {
    quad_double_sided_rect(size, size)
}

fn square_frame(outer: f32, thickness: f32) -> Geo {
    let h = outer * 0.5 - thickness * 0.5;
    let mut geo: Geo = (Vec::new(), Vec::new(), Vec::new());
    geo = merge(geo, translate_geo(quad_double_sided_rect(outer, thickness), [0.0, h, 0.0]));
    geo = merge(geo, translate_geo(quad_double_sided_rect(outer, thickness), [0.0, -h, 0.0]));
    geo = merge(geo, translate_geo(quad_double_sided_rect(thickness, outer), [h, 0.0, 0.0]));
    geo = merge(geo, translate_geo(quad_double_sided_rect(thickness, outer), [-h, 0.0, 0.0]));
    geo
}

fn arrow_geo() -> Geo {
    merge(cylinder(0.018, 0.6, 10), cone(0.05, 0.16, 0.6, 14))
}
fn scale_arrow_geo() -> Geo {
    merge(cylinder(0.018, 0.42, 10), cube(0.07, [0.0, 0.0, 0.42]))
}
fn ring_geo() -> Geo {
    torus(0.85, 0.014, 48, 8)
}
fn outer_ring_geo() -> Geo {
    torus(1.05, 0.012, 48, 6)
}
fn plane_geo() -> Geo {
    translate_geo(quad_double_sided(0.16), [0.24, 0.24, 0.0])
}
fn uniform_scale_geo() -> Geo {
    cube(0.06, [0.0, 0.0, 0.0])
}
fn uniform_translate_geo() -> Geo {
    square_frame(0.14, 0.012)
}

pub(crate) fn geometry_for(mode: GizmoMode, axis: Axis) -> Option<Geo> {
    use super::math::{rot_x_neg90, rot_y90};
    use Axis::*;
    use GizmoMode::*;
    Some(match (mode, axis) {
        (Translate, X) => apply_rot(arrow_geo(), rot_y90),
        (Translate, Y) => apply_rot(arrow_geo(), rot_x_neg90),
        (Translate, Z) => arrow_geo(),
        (Translate, XY) => plane_geo(),
        (Translate, XZ) => apply_rot(plane_geo(), rot_x_neg90),
        (Translate, YZ) => apply_rot(plane_geo(), rot_y90),
        (Translate, XYZ) => uniform_translate_geo(),

        (Scale, X) => apply_rot(scale_arrow_geo(), rot_y90),
        (Scale, Y) => apply_rot(scale_arrow_geo(), rot_x_neg90),
        (Scale, Z) => scale_arrow_geo(),
        (Scale, XYZ) => uniform_scale_geo(),

        (Rotate, X) => apply_rot(ring_geo(), rot_y90),
        (Rotate, Y) => apply_rot(ring_geo(), rot_x_neg90),
        (Rotate, Z) => ring_geo(),
        (Rotate, XYZ) => outer_ring_geo(),

        _ => return None,
    })
}

pub(crate) fn all_axes_for(mode: GizmoMode) -> &'static [Axis] {
    match mode {
        GizmoMode::Translate => &[Axis::X, Axis::Y, Axis::Z, Axis::XY, Axis::XZ, Axis::YZ, Axis::XYZ],
        GizmoMode::Rotate => &[Axis::X, Axis::Y, Axis::Z, Axis::XYZ],
        GizmoMode::Scale => &[Axis::X, Axis::Y, Axis::Z, Axis::XYZ],
    }
}