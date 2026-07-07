pub(crate) mod anim_sim_editor;
pub(crate) mod discover;
pub(crate) mod edit_camera;
pub(crate) mod grab_pose_editor;
pub(crate) mod input;
pub(crate) mod layout;
pub(crate) mod nav;
pub(crate) mod orbit_camera;
pub(crate) mod picking;
pub(crate) mod render;
pub(crate) mod scene_bridge;
pub(crate) mod setup;
pub(crate) mod snap;

use std::collections::HashMap;
use std::path::PathBuf;
use std::sync::Arc;
use std::time::Instant;

use wgpu::{Instance, InstanceDescriptor, Surface, SurfaceConfiguration};
use winit::keyboard::ModifiersState;
use winit::window::Window;

use space_soup::renderer::mesh_pipeline::ModelUniform;
use space_soup::renderer::{Camera, GltfMesh, Renderer};
use space_soup::ui2d::Overlay;
use space_soup_engine::{GameRuntime, Hand};

use agate::{AMouseButton, TextEditor, Ui};

use crate::network::SharedPacket;
use crate::transform_gizmo::{GizmoAssets, TransformGizmo};

use discover::{discover_json, discover_models, game_dir};
use edit_camera::EditCamera;

#[derive(Clone, Copy, PartialEq, Eq)]
pub(crate) enum ViewMode {
    PlayerView,
    FirstPerson,
    RenderView,
    Edit,
}

#[derive(Clone, Copy, PartialEq, Eq)]
pub(crate) enum GizmoPart {
    Orbit,
    Pan,
    Zoom,
}

#[derive(Clone, Copy, PartialEq, Eq)]
pub(crate) enum EditorTool {
    Select,
    Rigging,
    Snap,
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub(crate) enum EditTarget {
    SceneFile,
    ObjectScript(String),
}

pub(crate) struct App {
    pub(crate) window: Option<Arc<Window>>,
    pub(crate) instance: Instance,
    pub(crate) surface: Option<Surface<'static>>,
    pub(crate) config: Option<SurfaceConfiguration>,

    pub(crate) renderer: Option<Renderer>,
    pub(crate) overlay: Option<Overlay>,
    pub(crate) camera: Camera,

    pub(crate) ui: Option<Ui>,

    pub(crate) mouse_pos: (f32, f32),
    pub(crate) mouse_pressed: Vec<AMouseButton>,
    pub(crate) mouse_released: Vec<AMouseButton>,
    pub(crate) mouse_held: Vec<AMouseButton>,
    pub(crate) scroll_y: f32,

    pub(crate) model_scroll_y: f32,
    pub(crate) text_input: String,
    pub(crate) named_keys: Vec<agate::input::NamedKey>,
    pub(crate) mods: ModifiersState,

    pub(crate) packet: SharedPacket,
    pub(crate) runtime: GameRuntime,
    pub(crate) last_tick: Instant,
    pub(crate) scale: f32,

    pub(crate) mesh_cache: HashMap<String, (GltfMesh, ModelUniform)>,
    pub(crate) mesh_base_half_size: HashMap<String, glam::Vec3>,
    pub(crate) debug_meshes: Vec<(GltfMesh, ModelUniform)>,

    pub(crate) view_mode: ViewMode,
    pub(crate) edit_camera: EditCamera,
    pub(crate) last_world_head: glam::Vec3,

    pub(crate) selected_object: Option<String>,
    pub(crate) moving_object: bool,
    pub(crate) move_anchor_offset: glam::Vec3,
    pub(crate) last_click_time: Option<Instant>,
    pub(crate) last_clicked_object: Option<String>,
    pub(crate) dragging_new_model: Option<PathBuf>,
    pub(crate) ghost_preview: Option<glam::Vec3>,
    pub(crate) gizmo_drag: Option<GizmoPart>,
    pub(crate) press_in_chrome: bool,
    pub(crate) last_mouse_pos: (f32, f32),
    pub(crate) left_down: bool,
    pub(crate) dragged: bool,

    pub(crate) scene_dirty: bool,

    pub(crate) xform_gizmo: TransformGizmo,
    pub(crate) gizmo_assets: Option<GizmoAssets>,
    pub(crate) gizmo_dragging: bool,

    pub(crate) editor: TextEditor,
    pub(crate) editing: Option<EditTarget>,
    pub(crate) editor_focused: bool,
    pub(crate) selected_file: Option<usize>,

    pub(crate) nav_scenes_open: bool,
    pub(crate) nav_objects_open: bool,

    /// Measured height of the inspector's object cards; drives its scrollbar
    /// so short windows can still reach the bottom buttons.
    pub(crate) inspector_content_height: f32,

    pub(crate) files_discovered: Vec<PathBuf>,
    pub(crate) available_models: Vec<PathBuf>,

    pub(crate) tool: EditorTool,
    pub(crate) rig_selection: Vec<String>,
    pub(crate) snap_hand: Hand,
    pub(crate) snap_selected_joint: Option<usize>,
    pub(crate) snap_joint_frame: Vec<snap::SnapJoint>,

    pub(crate) grab_pose_editor: Option<grab_pose_editor::GrabPoseEditorState>,
    pub(crate) grab_pose_gizmo: TransformGizmo,

    pub(crate) anim_sim_editor: Option<anim_sim_editor::AnimSimEditorState>,
    /// Cross-object clipboards so animations/keyframes survive closing and
    /// reopening the anim-sim editor on another object.
    pub(crate) anim_clipboard: Option<space_soup_engine::Animation>,
    pub(crate) keyframe_clipboard: Option<space_soup_engine::Keyframe>,
}

impl App {
    pub(crate) fn new(packet: SharedPacket) -> Self {
        let dir = game_dir();
        let runtime = GameRuntime::load(&dir)
            .unwrap_or_else(|e| panic!("space_soup_editor cannot load game: {e}"));
        let files = discover_json(&dir);
        let models = discover_models(&dir);

        Self {
            window: None,
            instance: Instance::new(&InstanceDescriptor::default()),
            surface: None,
            config: None,
            renderer: None,
            overlay: None,
            camera: Camera::new(1.0),
            ui: None,

            mouse_pos: (0.0, 0.0),
            mouse_pressed: Vec::new(),
            mouse_released: Vec::new(),
            mouse_held: Vec::new(),
            scroll_y: 0.0,
            model_scroll_y: 0.0,
            text_input: String::new(),
            named_keys: Vec::new(),
            mods: ModifiersState::empty(),

            packet,
            runtime,
            last_tick: Instant::now(),
            scale: 1.0,
            mesh_cache: HashMap::new(),
            mesh_base_half_size: HashMap::new(),
            debug_meshes: Vec::new(),

            view_mode: ViewMode::PlayerView,
            edit_camera: EditCamera::new(glam::Vec3::new(0.0, 1.2, 0.0)),
            last_world_head: glam::Vec3::new(0.0, 1.2, 0.0),

            selected_object: None,
            moving_object: false,
            move_anchor_offset: glam::Vec3::ZERO,
            last_click_time: None,
            last_clicked_object: None,
            dragging_new_model: None,
            ghost_preview: None,
            gizmo_drag: None,
            press_in_chrome: false,
            last_mouse_pos: (0.0, 0.0),
            left_down: false,
            dragged: false,

            scene_dirty: false,

            xform_gizmo: TransformGizmo::new(),
            gizmo_assets: None,
            gizmo_dragging: false,

            editor: TextEditor::empty(),
            editing: None,
            editor_focused: false,
            selected_file: None,

            nav_scenes_open: true,
            nav_objects_open: true,

            inspector_content_height: 900.0,

            files_discovered: files,
            available_models: models,

            tool: EditorTool::Select,
            rig_selection: Vec::new(),
            snap_hand: Hand::Right,
            snap_selected_joint: None,
            snap_joint_frame: Vec::new(),

            grab_pose_editor: None,
            grab_pose_gizmo: TransformGizmo::new(),

            anim_sim_editor: None,
            anim_clipboard: None,
            keyframe_clipboard: None,
        }
    }

    pub(crate) fn win_size(&self) -> (f32, f32) {
        self.window
            .as_ref()
            .map(|w| {
                let s = w.inner_size();
                (s.width as f32, s.height as f32)
            })
            .unwrap_or((0.0, 0.0))
    }

    pub(crate) fn redraw_now(&self) {
        if let Some(w) = &self.window {
            w.request_redraw();
        }
    }
}
