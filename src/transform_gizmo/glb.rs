use space_soup::renderer::Color3;

use super::geometry::Geo;

fn srgb_to_linear(c: u8) -> f32 {
    let c = c as f32 / 255.0;
    if c <= 0.04045 {
        c / 12.92
    } else {
        ((c + 0.055) / 1.055).powf(2.4)
    }
}

pub(crate) fn write_glb(geo: &Geo, color: Color3) -> Vec<u8> {
    let (positions, normals, indices) = geo;
    let mut min = [f32::MAX; 3];
    let mut max = [f32::MIN; 3];
    for p in positions {
        for k in 0..3 {
            min[k] = min[k].min(p[k]);
            max[k] = max[k].max(p[k]);
        }
    }

    let mut bin: Vec<u8> = Vec::new();
    let pos_offset = bin.len() as u32;
    for p in positions {
        for v in p {
            bin.extend_from_slice(&v.to_le_bytes());
        }
    }
    while bin.len() % 4 != 0 {
        bin.push(0);
    }
    let norm_offset = bin.len() as u32;
    for n in normals {
        for v in n {
            bin.extend_from_slice(&v.to_le_bytes());
        }
    }
    while bin.len() % 4 != 0 {
        bin.push(0);
    }
    let idx_offset = bin.len() as u32;
    for i in indices {
        bin.extend_from_slice(&i.to_le_bytes());
    }
    while bin.len() % 4 != 0 {
        bin.push(0);
    }

    let pos_len = (positions.len() * 12) as u32;
    let norm_len = (normals.len() * 12) as u32;
    let idx_len = (indices.len() * 2) as u32;

    let r = srgb_to_linear(color.0);
    let g = srgb_to_linear(color.1);
    let b = srgb_to_linear(color.2);
    let a = color.3 as f32 / 255.0;

    let json = format!(
        concat!(
            "{{\"asset\":{{\"version\":\"2.0\"}},",
            "\"buffers\":[{{\"byteLength\":{bin_len}}}],",
            "\"bufferViews\":[",
            "{{\"buffer\":0,\"byteOffset\":{pos_offset},\"byteLength\":{pos_len},\"target\":34962}},",
            "{{\"buffer\":0,\"byteOffset\":{norm_offset},\"byteLength\":{norm_len},\"target\":34962}},",
            "{{\"buffer\":0,\"byteOffset\":{idx_offset},\"byteLength\":{idx_len},\"target\":34963}}",
            "],",
            "\"accessors\":[",
            "{{\"bufferView\":0,\"componentType\":5126,\"count\":{vert_count},\"type\":\"VEC3\",",
            "\"min\":[{minx},{miny},{minz}],\"max\":[{maxx},{maxy},{maxz}]}},",
            "{{\"bufferView\":1,\"componentType\":5126,\"count\":{vert_count},\"type\":\"VEC3\"}},",
            "{{\"bufferView\":2,\"componentType\":5123,\"count\":{idx_count},\"type\":\"SCALAR\"}}",
            "],",
            "\"materials\":[{{\"pbrMetallicRoughness\":{{\"baseColorFactor\":[{r},{g},{b},{a}],",
            "\"metallicFactor\":0.0,\"roughnessFactor\":1.0}}}}],",
            "\"meshes\":[{{\"primitives\":[{{\"attributes\":{{\"POSITION\":0,\"NORMAL\":1}},",
            "\"indices\":2,\"material\":0,\"mode\":4}}]}}],",
            "\"nodes\":[{{\"mesh\":0}}],",
            "\"scenes\":[{{\"nodes\":[0]}}],",
            "\"scene\":0}}",
        ),
        bin_len = bin.len(),
        pos_offset = pos_offset, pos_len = pos_len,
        norm_offset = norm_offset, norm_len = norm_len,
        idx_offset = idx_offset, idx_len = idx_len,
        vert_count = positions.len(), idx_count = indices.len(),
        minx = min[0], miny = min[1], minz = min[2],
        maxx = max[0], maxy = max[1], maxz = max[2],
        r = r, g = g, b = b, a = a,
    );

    let mut json_bytes = json.into_bytes();
    while json_bytes.len() % 4 != 0 {
        json_bytes.push(b' ');
    }

    let total_len = 12 + 8 + json_bytes.len() + 8 + bin.len();
    let mut out = Vec::with_capacity(total_len);
    out.extend_from_slice(b"glTF");
    out.extend_from_slice(&2u32.to_le_bytes());
    out.extend_from_slice(&(total_len as u32).to_le_bytes());

    out.extend_from_slice(&(json_bytes.len() as u32).to_le_bytes());
    out.extend_from_slice(&0x4E4F_534Au32.to_le_bytes());
    out.extend_from_slice(&json_bytes);

    out.extend_from_slice(&(bin.len() as u32).to_le_bytes());
    out.extend_from_slice(&0x004E_4942u32.to_le_bytes());
    out.extend_from_slice(&bin);

    out
}
