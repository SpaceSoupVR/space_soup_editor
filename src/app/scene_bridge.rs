//! Bridge between the engine's `GameObject` scene model and the editor's
//! selection/gizmo/inspector code. The engine's types are already
//! serde-friendly and `Clone`, so the editor edits `GameObject`s in the
//! live `runtime.scene_mut()` directly rather than maintaining a parallel
//! object list.

use std::path::Path;

use glam::{Quat, Vec3};

use space_soup::renderer::GltfMesh;
use space_soup_engine::{Color3, CuboidStyle, CuboidDef, GameObject, MeshRef, Scene};

pub(crate) fn new_object(id: String, pos: Vec3, half: Vec3, mesh_path: Option<String>) -> GameObject {
    GameObject {
        id,
        cuboid: CuboidDef {
            position: pos,
            half_size: half,
            rotation: Quat::IDENTITY,
            color: Color3(255, 255, 255, 255),
            wire_color: Color3(200, 200, 255, 255),
            style: if mesh_path.is_some() { CuboidStyle::Wireframe } else { CuboidStyle::Solid },
        },
        mesh: mesh_path.map(|path| MeshRef {
            path,
            scale: Vec3::ONE,
            rotation_offset: Quat::IDENTITY,
        }),
        is_trigger: false,
        hidden: false,
        script: None,
        animations: Vec::new(),
        rig_attachment: None,
        grip_pose_legacy: None,
        grip_pose_left: None,
        grip_pose_right: None,
        rigid_body: None,
        grip_points: Vec::new(),
    }
}

pub(crate) fn unique_id(scene: &Scene, base: &str) -> String {
    if scene.find_object(base).is_none() {
        return base.to_string();
    }
    let mut n = 2;
    loop {
        let candidate = format!("{base}_{n}");
        if scene.find_object(&candidate).is_none() {
            return candidate;
        }
        n += 1;
    }
}

pub(crate) fn save_scene(scene: &Scene, game_dir: &Path, scene_name: &str) -> anyhow::Result<std::path::PathBuf> {
    let path = game_dir.join("scenes").join(format!("{scene_name}.json"));
    scene.save(&path)?;
    Ok(path)
}

pub(crate) fn object_script(scene: &Scene, id: &str) -> String {
    scene.find_object(id)
        .and_then(|o| o.script.clone())
        .unwrap_or_default()
}

pub(crate) fn set_object_script(scene: &mut Scene, id: &str, text: String) {
    if let Some(obj) = scene.find_object_mut(id) {
        obj.script = Some(text);
    }
}

/// Computes a mesh's bounding half-size from its raw vertex data, in the
/// mesh's own local space (before any GltfMesh.scale/rotation/position is
/// applied) — i.e. the mesh's size "at scale = 1.0". Used only when the
/// gizmo's Scale mode needs a baseline to derive a scale *ratio* for the
/// mesh from a newly-set half_size, and when placing a freshly dropped
/// model. Never used to override values already saved in a scene file —
/// whatever's in the JSON (half_size and mesh.scale) is authoritative and
/// rendered as-is.
pub(crate) fn mesh_base_half_size(gltf: &GltfMesh) -> Vec3 {
    let mut min = Vec3::splat(f32::MAX);
    let mut max = Vec3::splat(f32::MIN);
    let mut found_any = false;

    for prim in &gltf.primitives {
        for v in &prim.vertices {
            let p = Vec3::from(v.position);
            min = min.min(p);
            max = max.max(p);
            found_any = true;
        }
    }

    if !found_any {
        return Vec3::splat(0.25);
    }
    ((max - min) * 0.5).max(Vec3::splat(0.01))
}

const DEFAULT_VOXEL_SIZE: f32 = 0.05;

fn unique_voxel_filename(models_dir: &Path, stem: &str) -> String {
    let candidate = format!("{stem}_voxel.glb");
    if !models_dir.join(&candidate).exists() {
        return candidate;
    }
    let mut n = 2;
    loop {
        let candidate = format!("{stem}_voxel_{n}.glb");
        if !models_dir.join(&candidate).exists() {
            return candidate;
        }
        n += 1;
    }
}

pub(crate) fn voxelize_object(scene: &mut Scene, game_dir: &Path, id: &str) -> anyhow::Result<String> {
    let src_rel_path = scene.find_object(id)
        .and_then(|o| o.mesh.as_ref().map(|m| m.path.clone()))
        .ok_or_else(|| anyhow::anyhow!("object '{id}' has no mesh to voxelize"))?;

    let src_full_path = game_dir.join(&src_rel_path);
    let models_dir = game_dir.join("models");
    std::fs::create_dir_all(&models_dir)?;

    let stem = std::path::Path::new(&src_rel_path)
        .file_stem()
        .map(|s| s.to_string_lossy().to_string())
        .unwrap_or_else(|| "model".to_string());
    let out_filename = unique_voxel_filename(&models_dir, &stem);
    let out_full_path = models_dir.join(&out_filename);
    let out_rel_path = format!("models/{out_filename}");

    space_soup::voxelize::gltf_to_cuboid_glb(&src_full_path, &out_full_path, DEFAULT_VOXEL_SIZE)?;

    let src_pos = scene.find_object(id).map(|o| o.cuboid.position).unwrap_or(Vec3::ZERO);
    let src_half = scene.find_object(id).map(|o| o.cuboid.half_size).unwrap_or(Vec3::splat(0.25));

    let new_id = unique_id(scene, &format!("{stem}_voxel"));
    let new_pos = src_pos + Vec3::new(src_half.x * 2.2, 0.0, 0.0);
    let obj = new_object(new_id.clone(), new_pos, src_half, Some(out_rel_path));
    scene.objects.push(obj);

    Ok(new_id)
}