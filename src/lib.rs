use bevy::input::mouse::{MouseMotion, MouseWheel};
use bevy::prelude::*;
use bevy::window::PrimaryWindow;
use bevy_inspector_egui::bevy_egui::egui;
use bevy_egui::EguiContexts;
use bevy_transform_gizmo::{GizmoPartsEnabled, GizmoPickSource, GizmoSettings};

#[derive(Default)]
pub struct CameraPlugin;

#[derive(Resource)]
struct CameraData {
    transform_orientation: GizmoSpace,
    ui_show_transform_or_scale: TransformOrScale,
}

#[derive(Clone, Hash, PartialEq, Eq, Debug)]
struct CameraRunCriteria;

#[derive(Debug, Hash, PartialEq, Eq, Clone, SystemSet)]
pub enum CameraSystem {
    PanOrbit,
    Adjust,
    UpdateSpace,
    UISytem,
}

#[derive(Debug, PartialEq, Resource, Copy, Clone)]
enum GizmoSpace {
    Global,
    Local,
    Screen,
}

/// Tags an entity as capable of panning and orbiting.
#[derive(Component)]
pub struct PanOrbitCamera {
    /// The "focus point" to orbit around. It is automatically updated when panning the camera
    pub focus: Vec3,
    pub radius: f32,
    pub upside_down: bool,
}

impl Default for PanOrbitCamera {
    fn default() -> Self {
        PanOrbitCamera {
            focus: Vec3::ZERO,
            radius: 5.0,
            upside_down: false,
        }
    }
}

impl Plugin for CameraPlugin {
    fn build(&self, app: &mut App) {
        app.add_plugin(bevy_transform_gizmo::TransformGizmoPlugin)
            .add_systems(
                (pan_orbit_camera, center_selection)
                    .chain()
                    .in_base_set(CoreSet::Update)
                    //.run_if(plugin_enabled),
            )
            .add_systems(
                (ui_system, update_gizmo_space).chain()
            )
            .insert_resource(CameraData {
                transform_orientation: GizmoSpace::Global,
                ui_show_transform_or_scale: TransformOrScale::Transform,
            });
    }
}

fn center_selection(
    selection: Query<(&Transform, &bevy_mod_picking::Selection)>,
    mut camera: Query<(&mut PanOrbitCamera, &Transform)>,
    keyboard_input: Res<Input<KeyCode>>,
) {
    if !selection.iter().any(|(_, selection)| selection.selected()) {
        return;
    }

    if keyboard_input.just_released(KeyCode::Period) {
        let mut total = Vec3::ZERO;
        let mut point_count = 0;
        for (transform, selection) in &selection {
            if selection.selected() {
                total += transform.translation;
                point_count += 1;
            }
        }
        let center = total / point_count as f32;
        let (mut camera, camera_transform) = camera.single_mut();
        camera.radius = (camera_transform.translation - center).length();
        camera.focus = center;
    }
}

/// Disable this plugin and bevy_mod_picking plugin if the cursor is in a egui window
fn plugin_enabled(
    mut egui_contexts: EguiContexts,
    mut state: ResMut<bevy_mod_picking::PickingPluginsState>,
) -> bool {
    // don't adjust camera if the mouse pointer in over an egui window
    let ctx = egui_contexts.ctx_mut();
    let pointer_over_area = ctx.is_pointer_over_area();
    let using_pointer = ctx.is_using_pointer();
    let wants_pointer = ctx.wants_pointer_input();

    if wants_pointer || pointer_over_area || using_pointer {
        state.enable_picking = false;
        //state.enable_highlighting = false;
        state.enable_interacting = false;
        false
    } else {
        state.enable_picking = true;
        //state.enable_highlighting = true;
        state.enable_interacting = true;
        true
    }
}

#[derive(Debug, PartialEq, Copy, Clone)]
enum TransformOrScale {
    Transform,
    Scale,
    Neither,
}

fn ui_system(
    mut egui_context: EguiContexts,
    mut app_assets: ResMut<CameraData>,
    mut enabled_systems: ResMut<GizmoPartsEnabled>,
) {
    let mut selected = app_assets.transform_orientation;
    let mut showing = app_assets.ui_show_transform_or_scale;
    egui::Window::new("Transform Gizmo").show(egui_context.ctx_mut(), |ui| {
        egui::ComboBox::from_label("Orientation")
            .selected_text(format!("{:?}", selected))
            .show_ui(ui, |ui| {
                ui.selectable_value(&mut selected, GizmoSpace::Global, "Global");
                ui.selectable_value(&mut selected, GizmoSpace::Local, "Local");
                ui.selectable_value(&mut selected, GizmoSpace::Screen, "Screen");
            });
        ui.radio_value(&mut showing, TransformOrScale::Scale, "Scale");
        ui.radio_value(&mut showing, TransformOrScale::Transform, "Transform");
        ui.radio_value(&mut showing, TransformOrScale::Neither, "Neither");
        match showing {
            TransformOrScale::Transform => {
                enabled_systems.translate_arrows = true;
                enabled_systems.scale = false;
            }
            TransformOrScale::Scale => {
                enabled_systems.translate_arrows = false;
                enabled_systems.scale = true;
            }
            TransformOrScale::Neither => {
                enabled_systems.translate_arrows = false;
                enabled_systems.scale = false;
            }
        }
        ui.checkbox(&mut enabled_systems.rotate, "Rotate");
        ui.checkbox(&mut enabled_systems.translate_planes, "Planes");
    });
    app_assets.ui_show_transform_or_scale = showing;
    app_assets.transform_orientation = selected;
    // update_gizmo_space(selection, selected, gizmo, camera);
}

fn pan_orbit_camera(
    window: Query<&Window, With<PrimaryWindow>>,
    mut ev_motion: EventReader<MouseMotion>,
    mut ev_scroll: EventReader<MouseWheel>,
    input_mouse: Res<Input<MouseButton>>,
    mut query: Query<(&mut PanOrbitCamera, &mut Transform, &Projection)>,
    keyboard_input: Res<Input<KeyCode>>,
) {

    let window = match window.get_single() {
        Ok(window) => {
            window
        }
        Err(err) => {
            error!("couldn't get primary window {}", err);
            return;
        }
    };

    // change input mapping for orbit and panning here
    let orbit_button = MouseButton::Middle;
    let pan_button = MouseButton::Middle;
    let pan_key_left = KeyCode::LShift;
    let pan_key_right = KeyCode::RShift;

    let mut pan = Vec2::ZERO;
    let mut rotation_move = Vec2::ZERO;
    let mut scroll = 0.0;
    let mut orbit_button_changed = false;

    if input_mouse.pressed(orbit_button)
        && !(keyboard_input.pressed(pan_key_right) || keyboard_input.pressed(pan_key_left))
    {
        for ev in ev_motion.iter() {
            rotation_move += ev.delta;
        }
    } else if input_mouse.pressed(pan_button)
        && (keyboard_input.pressed(pan_key_right) || keyboard_input.pressed(pan_key_left))
    {
        // Pan only if we're not rotating at the moment
        for ev in ev_motion.iter() {
            pan += ev.delta;
        }
    }
    for ev in ev_scroll.iter() {
        scroll += ev.y;
    }
    if input_mouse.just_released(orbit_button) || input_mouse.just_pressed(orbit_button) {
        orbit_button_changed = true;
    }

    for (mut pan_orbit, mut transform, projection) in query.iter_mut() {
        if orbit_button_changed {
            // only check for upside down when orbiting started or ended this frame
            // if the camera is "upside" down, panning horizontally would be inverted, so invert the input to make it correct
            let up = transform.rotation * Vec3::Y;
            pan_orbit.upside_down = up.y <= 0.0;
        }

        let mut any = false;
        if rotation_move.length_squared() > 0.0 {
            any = true;
            let primary_window_size = Vec2::new(window.width() as f32, window.height() as f32);
            let delta_x = {
                let delta = rotation_move.x / primary_window_size.x * std::f32::consts::PI * 2.0;
                if pan_orbit.upside_down {
                    -delta
                } else {
                    delta
                }
            };
            let delta_y = rotation_move.y / primary_window_size.y * std::f32::consts::PI;
            let yaw = Quat::from_rotation_y(-delta_x);
            let pitch = Quat::from_rotation_x(-delta_y);
            transform.rotation = yaw * transform.rotation; // rotate around global y axis
            transform.rotation = transform.rotation * pitch; // rotate around local x axis

        } else if pan.length_squared() > 0.0 {

            any = true;
            // make panning distance independent of resolution and FOV,
            //let window = get_primary_window_size(&windows);
            let primary_window_size = Vec2::new(window.width() as f32, window.height() as f32);

            if let Projection::Perspective(projection) = projection {
                pan *= Vec2::new(projection.fov * projection.aspect_ratio, projection.fov) / primary_window_size;
            }
            // translate by local axes
            let right = transform.rotation * Vec3::X * -pan.x;
            let up = transform.rotation * Vec3::Y * pan.y;
            // make panning proportional to distance away from focus point
            let translation = (right + up) * pan_orbit.radius;
            pan_orbit.focus += translation;
        } else if scroll.abs() > 0.0 {
            any = true;
            pan_orbit.radius -= scroll * pan_orbit.radius * 0.002;
            // dont allow zoom to reach zero or you get stuck
            pan_orbit.radius = f32::max(pan_orbit.radius, 0.05);
        }

        if any {
            // emulating parent/child to make the yaw/y-axis rotation behave like a turntable
            // parent = x and y rotation
            // child = z-offset
            let rot_matrix = Mat3::from_quat(transform.rotation);
            transform.translation =
                pan_orbit.focus + rot_matrix.mul_vec3(Vec3::new(0.0, 0.0, pan_orbit.radius));
        }
    }
}

/*
fn adjust(mut query: Query<(&mut PanOrbitCamera, &mut Transform)>) {
    for (pan_orbit, mut transform) in query.iter_mut() {
        let rot_matrix = Mat3::from_quat(transform.rotation);
        transform.translation =
            pan_orbit.focus + rot_matrix.mul_vec3(Vec3::new(0.0, 0.0, pan_orbit.radius));
    }
}
*/

fn update_gizmo_space(
    app_assets: ResMut<CameraData>,
    selection: Query<(&GlobalTransform, &bevy_mod_picking::Selection)>,
    mut gizmo: ResMut<GizmoSettings>,
    camera: Query<&Transform, With<GizmoPickSource>>,
) {
    let gizmo_space = app_assets.transform_orientation;
    let selected = selection.iter().filter_map(|(transform, selection)| {
        if selection.selected() {
            Some(transform)
        } else {
            None
        }
    });

    match gizmo_space {
        GizmoSpace::Local => {
            for transform in selected.take(1) {
                gizmo.alignment_rotation = Quat::from_mat4(&transform.compute_matrix());
            }
        }
        GizmoSpace::Global => {
            for _ in selected.take(1) {
                gizmo.alignment_rotation = Quat::IDENTITY;
            }
        }
        GizmoSpace::Screen => {
            let cam_transform = match camera.get_single() {
                Ok(x) => x,
                Err(_) => return,
            };
            let direction = cam_transform.local_z();
            let rotation = Quat::from_mat3(&Mat3::from_cols(
                direction.cross(cam_transform.local_y()),
                direction,
                cam_transform.local_y(),
            ));
            for _ in selected.take(1) {
                gizmo.alignment_rotation = rotation;
            }
            // from the mesh/mod.rs for the center sphere
            /*TransformGizmoInteraction::TranslatePlane {
                original: Vec3::ZERO,
                normal: Vec3::Z,
            },*/
        }
    }
}
