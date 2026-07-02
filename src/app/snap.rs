use std::collections::HashMap;

use glam::{Mat4, Quat, Vec3};

use space_soup::renderer::mesh::GltfSkin;
use space_soup::renderer::{Camera, GltfMesh, MeshInstance, Renderer};
use space_soup_engine::{GameObject, GripPoseDef, Scene};

use crate::transform_gizmo::{GizmoAssets, GizmoMode, TransformGizmo};

use super::picking::ray_aabb_hit;
use super::{App, EditorTool};

pub(crate) const FINGER_BONES: [&str; 16] = [
    "thumb1", "thumb2", "thumb3",
    "index1", "index2", "index3",
    "middle1", "middle2", "middle3",
    "ring1", "ring2", "ring3",
    "pinky0", "pinky1", "pinky2", "pinky3",
];

#[derive(Clone)]
pub(crate) struct SnapJoint {
    pub name: String,
    pub open_pos: Vec3,
    pub closed_pos: Vec3,
    pub current_pos: Vec3,
}

pub(crate) fn compute_snap_joints(
    skin: &GltfSkin,
    root: Mat4,
    finger_curl: &HashMap<String, f32>,
) -> Vec<SnapJoint> {
    let open_local = skin.blended_local_pose(0, 1, |_| 0.0);
    let closed_local = skin.blended_local_pose(0, 1, |_| 1.0);
    let current_local = skin.blended_local_pose(0, 1, |ji| {
        let name = GltfSkin::generic_joint_name(&skin.joint_names[ji]);
        finger_curl.get(name).copied().unwrap_or(0.5)
    });

    let open_global = skin.hierarchical_transforms(&open_local);
    let closed_global = skin.hierarchical_transforms(&closed_local);
    let current_global = skin.hierarchical_transforms(&current_local);

    skin.joint_names.iter().enumerate()
        .filter_map(|(ji, name)| {
            let generic = GltfSkin::generic_joint_name(name);
            if !FINGER_BONES.contains(&generic) { return None; }
            Some(SnapJoint {
                name: generic.to_string(),
                open_pos: root.transform_point3(open_global[ji].transform_point3(Vec3::ZERO)),
                closed_pos: root.transform_point3(closed_global[ji].transform_point3(Vec3::ZERO)),
                current_pos: root.transform_point3(current_global[ji].transform_point3(Vec3::ZERO)),
            })
        })
        .collect()
}

pub(crate) fn project_curl(open: Vec3, closed: Vec3, dragged: Vec3) -> f32 {
    let seg = closed - open;
    let len_sq = seg.length_squared();
    if len_sq < 1e-10 { return 0.0; }
    ((dragged - open).dot(seg) / len_sq).clamp(0.0, 1.0)
}

pub(crate) fn grip_root(obj: &GameObject, grip: &GripPoseDef) -> Mat4 {
    let obj_mat = Mat4::from_rotation_translation(obj.cuboid.rotation, obj.cuboid.position);
    let offset_mat = Mat4::from_rotation_translation(
        Quat::from_array(grip.hand_offset_rot),
        Vec3::from(grip.hand_offset_pos),
    );
    obj_mat * offset_mat
}

pub(crate) fn seed_grip_pose(scene: &mut Scene, object_id: &str, hand_id: Option<&str>) {
    let hand_world = hand_id
        .and_then(|hid| scene.find_object(hid))
        .map(|h| (h.cuboid.position, h.cuboid.rotation));

    let Some(obj) = scene.find_object_mut(object_id) else { return };

    let (hand_offset_pos, hand_offset_rot) = match hand_world {
        Some((hpos, hrot)) => {
            let obj_mat = Mat4::from_rotation_translation(obj.cuboid.rotation, obj.cuboid.position);
            let hand_mat = Mat4::from_rotation_translation(hrot, hpos);
            let (_, rot, pos) = (obj_mat.inverse() * hand_mat).to_scale_rotation_translation();
            (pos.to_array(), rot.to_array())
        }
        None => ([0.0, 0.0, 0.0], Quat::IDENTITY.to_array()),
    };

    match &mut obj.grip_pose {
        Some(g) => {
            g.hand_offset_pos = hand_offset_pos;
            g.hand_offset_rot = hand_offset_rot;
        }
        None => {
            obj.grip_pose = Some(GripPoseDef {
                hand_offset_pos,
                hand_offset_rot,
                finger_curl: HashMap::new(),
            });
        }
    }
}

fn hand_glb_path(hand: space_soup_engine::Hand) -> &'static str {
    match hand {
        space_soup_engine::Hand::Left  => "models/left_hand.glb",
        space_soup_engine::Hand::Right => "models/right_hand.glb",
    }
}

#[allow(clippy::too_many_arguments)]
pub(crate) fn update_preview(
    renderer: &Renderer,
    mesh_cache: &mut HashMap<String, (GltfMesh, space_soup::renderer::mesh_pipeline::ModelUniform)>,
    game_dir: &std::path::Path,
    scene: &mut Scene,
    scene_dirty: &mut bool,
    tool: EditorTool,
    selected_object: Option<&str>,
    snap_hand: space_soup_engine::Hand,
    snap_selected_joint: &mut Option<usize>,
    snap_joint_frame: &mut Vec<SnapJoint>,
) {
    if tool != EditorTool::Snap {
        snap_joint_frame.clear();
        return;
    }

    let Some(obj_id) = selected_object.map(str::to_string) else {
        snap_joint_frame.clear();
        return;
    };

    if scene.find_object(&obj_id).map(|o| o.grip_pose.is_none()).unwrap_or(true) {
        seed_grip_pose(scene, &obj_id, None);
        *scene_dirty = true;
    }

    let hand_path = hand_glb_path(snap_hand);
    if !mesh_cache.contains_key(hand_path) {
        let full_path = game_dir.join(hand_path);
        match GltfMesh::load(&renderer.device, &renderer.queue, renderer.mesh_texture_layout(), &full_path) {
            Ok(mesh) => {
                let model_uniform = renderer.create_model_uniform();
                mesh_cache.insert(hand_path.to_string(), (mesh, model_uniform));
            }
            Err(e) => {
                log::warn!("space_soup_editor: Snap tool couldn't load {hand_path}: {e}");
                snap_joint_frame.clear();
                return;
            }
        }
    }

    let Some(obj) = scene.find_object(&obj_id) else {
        snap_joint_frame.clear();
        return;
    };
    let Some(grip) = obj.grip_pose.clone() else {
        snap_joint_frame.clear();
        return;
    };
    let root = grip_root(obj, &grip);

    let Some((mesh, _)) = mesh_cache.get(hand_path) else {
        snap_joint_frame.clear();
        return;
    };
    let Some(skin) = &mesh.skin else {
        log::warn!("space_soup_editor: {hand_path} has no skin — can't preview finger joints");
        snap_joint_frame.clear();
        return;
    };

    *snap_joint_frame = compute_snap_joints(skin, root, &grip.finger_curl);
    if snap_selected_joint.map(|i| i >= snap_joint_frame.len()).unwrap_or(false) {
        *snap_selected_joint = None;
    }
}

pub(crate) fn pick_joint_marker(joints: &[SnapJoint], origin: Vec3, dir: Vec3) -> Option<usize> {
    const HIT_RADIUS: f32 = 0.025;
    joints.iter().enumerate()
        .filter_map(|(i, j)| ray_aabb_hit(origin, dir, j.current_pos, Vec3::splat(HIT_RADIUS)).map(|t| (i, t)))
        .min_by(|a, b| a.1.partial_cmp(&b.1).unwrap())
        .map(|(i, _)| i)
}

pub(crate) fn collect_joint_gizmo_instances<'a>(
    xform_gizmo: &mut TransformGizmo,
    gizmo_assets: &'a mut Option<GizmoAssets>,
    camera: &Camera,
    viewport: (f32, f32),
    joint_pos: Option<Vec3>,
    is_dragging: bool,
) -> Vec<MeshInstance<'a>> {
    let Some(pos) = joint_pos else { return Vec::new() };
    xform_gizmo.mode = GizmoMode::Translate;
    if !is_dragging {
        xform_gizmo.set_position(pos);
    }
    let Some(assets) = gizmo_assets.as_mut() else { return Vec::new() };
    xform_gizmo.collect_mesh_instances(assets, camera, viewport)
}

pub(crate) fn apply_gizmo_drag_to_joint(app: &mut App) {
    let Some(obj_id) = app.selected_object.clone() else { return };
    let Some(idx) = app.snap_selected_joint else { return };
    let Some(joint) = app.snap_joint_frame.get(idx).cloned() else { return };
    let dragged = app.xform_gizmo.get_position();
    let t = project_curl(joint.open_pos, joint.closed_pos, dragged);

    if let Some(obj) = app.runtime.scene_mut().find_object_mut(&obj_id) {
        if let Some(grip) = &mut obj.grip_pose {
            grip.finger_curl.insert(joint.name.clone(), t);
            app.scene_dirty = true;
        }
    }
}
