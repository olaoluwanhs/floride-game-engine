// In Rust, declaring `mod` introduces another file in the same directory as a submodule.
// This is similar to importing a module in TypeScript, but in Rust, the root file
// (`main.rs` or `lib.rs`) explicitly declares the crate's module tree.
mod render;
mod scene;

use macroquad::prelude::{
    clear_background, get_frame_time, is_key_down, is_key_pressed, next_frame, screen_width,
    set_default_camera, vec2, KeyCode, BLACK,
};
use macroquad::ui::widgets::{Button, Window};
use macroquad::ui::{hash, root_ui};

use render::OrbitCamera;
use scene::Scene;

/// The physics simulation runs at a fixed frequency of 60 ticks per second (60 Hz).
///
/// Running physics at a fixed rate ensures consistent collisions, jumps, and velocities
/// regardless of whether the user has a 60 Hz, 144 Hz, or fluctuating monitor refresh rate.
const FIXED_TIME_STEP: f32 = 1.0 / 60.0;

/// Entry point for the visual demonstration application.
///
/// `#[macroquad::main(...)]` is an attribute macro that sets up the window and graphical
/// backend, wrapping our `async fn main()` so Macroquad can control the game loop.
#[macroquad::main("FPS Core Physics Showcase")]
async fn main() {
    // Initialize the scene. In Rust, functions that can fail return a `Result<T, E>`.
    // Instead of try/catch, we match on `Ok` (success) or `Err` (failure) to initialize
    // the scene state safely without crashing.
    let (mut scene, mut simulation_error) = match Scene::new() {
        Ok(scene) => (Some(scene), None),
        Err(error) => (None, Some(error)),
    };

    let mut camera = OrbitCamera::default();
    let mut accumulated_time = 0.0;
    let mut jump_queued = false;

    // Macroquad runs an asynchronous rendering loop. Each iteration processes one frame.
    loop {
        // `get_frame_time()` returns the elapsed wall-clock seconds since the previous frame.
        // We cap this delta to 0.1s (100ms) to prevent the "spiral of death" — a situation where
        // a sudden lag spike causes the physics loop below to run so many catch-up ticks
        // that it causes even more lag on the next frame.
        let frame_time = get_frame_time().min(0.1);
        camera.update(frame_time);
        clear_background(BLACK);

        if let Some(active_scene) = scene.as_mut() {
            // --- 1. ACCUMULATOR PATTERN FOR FIXED-TIMESTEP SIMULATION ---
            //
            // Rendering happens at variable frame rates (e.g. 60 FPS, 120 FPS), but physics
            // needs a constant time step. We accumulate elapsed frame time, and whenever we
            // have at least 1/60th of a second stored up, we advance the physics by one tick.
            accumulated_time += frame_time;

            // Discrete player inputs (like pressing Space to jump) can happen between frames.
            // We use bitwise OR (`|=`) so a jump press isn't forgotten if multiple frames pass
            // before the next fixed physics tick occurs.
            jump_queued |= is_key_pressed(KeyCode::Space);

            while accumulated_time >= FIXED_TIME_STEP {
                // Read WASD keys and rotate the direction to match the camera's viewing angle.
                let movement = read_movement_input(camera.yaw());

                // Advance the physics world by exactly one 60 Hz step.
                if let Err(error) = active_scene.tick(movement, jump_queued, FIXED_TIME_STEP) {
                    simulation_error = Some(error);
                    break;
                }

                // Reset the one-shot jump trigger and deduct the consumed time slice.
                jump_queued = false;
                accumulated_time -= FIXED_TIME_STEP;
            }

            // --- 2. RENDERING AND PRESENTATION ---
            //
            // If the simulation encountered an unrecoverable error, display it on screen.
            // Otherwise, fetch lightweight render snapshots from the scene and draw them.
            if let Some(error) = simulation_error.as_deref() {
                render::draw_error(error);
            } else {
                match active_scene.render_items() {
                    Ok(items) => {
                        render::draw_scene(&items, active_scene.ray_visuals(), &camera);
                    }
                    Err(error) => simulation_error = Some(error),
                }

                // Draw 3D Blender-style Bézier control polygon, knots, and tangent handles
                if let Some(tf) = active_scene.bezier_ramp_transform() {
                    let bz = active_scene.bezier_params();
                    render::draw_bezier_spline_gizmos(
                        &bz.segments,
                        &tf,
                        bz.selected_segment,
                        bz.selected_point,
                    );
                }

                // Switch back to 2D UI coordinates to draw the text overlay (HUD).
                set_default_camera();
                render::draw_hud(active_scene.status_lines(), screen_width());

                // Interactive Blender-Style Bézier Spline Editor Window
                let mut p = active_scene.bezier_params().clone();
                let mut changed = false;
                let mut teleport_ramp = false;
                let mut focus_spline = is_key_pressed(KeyCode::F);
                let mut focus_player = false;

                Window::new(
                    hash!("bezier_spline_editor"),
                    vec2(screen_width() - 360.0, 10.0),
                    vec2(350.0, 520.0),
                )
                .label("Bézier 3D Spline Editor (Blender-Style)")
                .ui(&mut root_ui(), |ui| {
                    ui.label(None, &format!("Spline Segments: {}", p.segments.len()));

                    // Focus & Frame Viewport Buttons
                    if Button::new("Focus Camera on Spline (F)").size(vec2(165.0, 22.0)).ui(ui) {
                        focus_spline = true;
                    }
                    ui.same_line(175.0);
                    if Button::new("Focus Player").size(vec2(150.0, 22.0)).ui(ui) {
                        focus_player = true;
                    }

                    ui.separator();

                    // Extrude or delete segments
                    if Button::new("+ Extrude Segment (Chain Next)").size(vec2(165.0, 22.0)).ui(ui) {
                        if let Some(last_seg) = p.segments.last().cloned() {
                            let next_seg = last_seg.extrude_continuation(4.0);
                            p.segments.push(next_seg);
                            p.selected_segment = p.segments.len() - 1;
                            p.selected_point = 3;
                            changed = true;
                        }
                    }
                    ui.same_line(175.0);
                    if Button::new("- Delete Segment").size(vec2(150.0, 22.0)).ui(ui) {
                        if p.segments.len() > 1 {
                            p.segments.pop();
                            p.selected_segment = p.selected_segment.min(p.segments.len() - 1);
                            changed = true;
                        }
                    }

                    ui.separator();

                    // Segment and Point Selection
                    if p.selected_segment >= p.segments.len() {
                        p.selected_segment = 0;
                    }

                    let cur_seg_idx = p.selected_segment;
                    ui.label(None, &format!("Selected: Segment #{}", cur_seg_idx + 1));
                    for i in 0..p.segments.len() {
                        let btn_label = format!("Seg {}", i + 1);
                        if Button::new(btn_label.as_str())
                            .size(vec2(60.0, 20.0))
                            .selected(p.selected_segment == i)
                            .ui(ui)
                        {
                            p.selected_segment = i;
                        }
                        if (i + 1) % 4 != 0 && i + 1 < p.segments.len() {
                            ui.same_line(68.0 * ((i % 4) + 1) as f32);
                        }
                    }

                    ui.label(None, "Active Point (Knots & Tangent Handles):");
                    let pt_names = ["Knot P0 (Start)", "Handle P1 (Out)", "Handle P2 (In)", "Knot P3 (End)"];
                    for (i, name) in pt_names.iter().enumerate() {
                        if Button::new(*name)
                            .size(vec2(155.0, 20.0))
                            .selected(p.selected_point == i)
                            .ui(ui)
                        {
                            p.selected_point = i;
                        }
                        if i % 2 == 0 {
                            ui.same_line(165.0);
                        }
                    }

                    ui.separator();

                    // Full 3D Coordinate editing for the selected point
                    let pt_idx = p.selected_point.min(3);
                    let seg = &mut p.segments[cur_seg_idx];
                    let pt_ref = match pt_idx {
                        0 => &mut seg.p0,
                        1 => &mut seg.p1,
                        2 => &mut seg.p2,
                        _ => &mut seg.p3,
                    };

                    ui.label(None, &format!("Move {} on ALL 3 AXES (X, Y, Z):", pt_names[pt_idx]));
                    let mut px = pt_ref[0];
                    let mut py = pt_ref[1];
                    let mut pz = pt_ref[2];

                    ui.slider(hash!("pt_x"), "X (Left/Right)", -15.0..15.0, &mut px);
                    ui.slider(hash!("pt_y"), "Y (Up/Down)", -5.0..15.0, &mut py);
                    ui.slider(hash!("pt_z"), "Z (Forward/Back)", -20.0..20.0, &mut pz);

                    if (px - pt_ref[0]).abs() > 0.005 || (py - pt_ref[1]).abs() > 0.005 || (pz - pt_ref[2]).abs() > 0.005 {
                        let delta_x = px - pt_ref[0];
                        let delta_y = py - pt_ref[1];
                        let delta_z = pz - pt_ref[2];
                        pt_ref[0] = px;
                        pt_ref[1] = py;
                        pt_ref[2] = pz;

                        // Continuity rule: If editing Knot P3 of segment i, sync Knot P0 of segment i+1
                        if pt_idx == 3 && cur_seg_idx + 1 < p.segments.len() {
                            p.segments[cur_seg_idx + 1].p0 = [px, py, pz];
                            p.segments[cur_seg_idx + 1].p1[0] += delta_x;
                            p.segments[cur_seg_idx + 1].p1[1] += delta_y;
                            p.segments[cur_seg_idx + 1].p1[2] += delta_z;
                        } else if pt_idx == 0 && cur_seg_idx > 0 {
                            p.segments[cur_seg_idx - 1].p3 = [px, py, pz];
                            p.segments[cur_seg_idx - 1].p2[0] += delta_x;
                            p.segments[cur_seg_idx - 1].p2[1] += delta_y;
                            p.segments[cur_seg_idx - 1].p2[2] += delta_z;
                        }
                        changed = true;
                    }

                    ui.separator();
                    ui.label(None, "Cross-Section Extrusion Profile:");
                    let mut hw = p.half_width;
                    ui.slider(hash!("bz_w"), "Half Width (m)", 0.25..4.0, &mut hw);
                    if (hw - p.half_width).abs() > 0.01 {
                        p.half_width = hw;
                        changed = true;
                    }

                    let mut hth = p.half_thickness;
                    ui.slider(hash!("bz_th"), "Half Thick (m)", 0.05..1.0, &mut hth);
                    if (hth - p.half_thickness).abs() > 0.01 {
                        p.half_thickness = hth;
                        changed = true;
                    }

                    let mut r = p.radius;
                    ui.slider(hash!("bz_r"), "Pipe Radius (m)", 0.1..2.0, &mut r);
                    if (r - p.radius).abs() > 0.01 {
                        p.radius = r;
                        changed = true;
                    }

                    ui.separator();
                    if Button::new("Teleport Player to Start of Spline").size(vec2(320.0, 24.0)).ui(ui) {
                        teleport_ramp = true;
                    }
                });

                if changed {
                    if let Err(error) = active_scene.update_bezier_shapes(p.clone()) {
                        simulation_error = Some(error);
                    }
                } else if p.selected_segment != active_scene.bezier_params().selected_segment
                    || p.selected_point != active_scene.bezier_params().selected_point
                {
                    active_scene.bezier_params_mut().selected_segment = p.selected_segment;
                    active_scene.bezier_params_mut().selected_point = p.selected_point;
                }

                if teleport_ramp {
                    if let Some(tf) = active_scene.bezier_ramp_transform() {
                        let first_p0 = p.segments[0].p0;
                        let target = game_types::Vec3::new(
                            tf.position.x + first_p0[0],
                            tf.position.y + first_p0[1] + 1.2,
                            tf.position.z + first_p0[2] - 1.5,
                        );
                        if let Err(error) = active_scene.teleport_player(target) {
                            simulation_error = Some(error);
                        }
                    }
                }

                if focus_spline {
                    if let Some(tf) = active_scene.bezier_ramp_transform() {
                        // Center camera on the midpoint of the active spline segment
                        let cur_seg = &p.segments[p.selected_segment.min(p.segments.len() - 1)];
                        let mid = cur_seg.point(0.5);
                        camera.target = macroquad::prelude::vec3(
                            tf.position.x + mid.x,
                            tf.position.y + mid.y,
                            tf.position.z + mid.z,
                        );
                        camera.settings.distance = 9.0;
                        camera.pitch = 0.45;
                    }
                }

                if focus_player {
                    if let Ok(pos) = active_scene.player_position() {
                        camera.target = macroquad::prelude::vec3(pos.x, pos.y + 0.8, pos.z);
                        camera.settings.distance = 5.5;
                        camera.pitch = 0.35;
                    }
                }
            }
        } else if let Some(error) = simulation_error.as_deref() {
            render::draw_error(error);
        }

        // Yield control back to Macroquad/OS until the next display frame is ready to draw.
        next_frame().await;
    }
}

/// Reads the WASD movement keys and converts them into a 3D world-space direction vector.
///
/// Moving in games should be intuitive: pressing 'W' should move the character towards
/// wherever the camera is pointing, not towards an arbitrary fixed world coordinate.
fn read_movement_input(camera_yaw: f32) -> game_types::Vec3 {
    let mut movement = game_types::Vec3::ZERO;

    // Collect 2D keyboard inputs along standard game axes:
    // A/D correspond to lateral (strafe) movement on X.
    // W/S correspond to forward/backward movement on Z.
    if is_key_down(KeyCode::A) {
        movement.x -= 1.0;
    }
    if is_key_down(KeyCode::D) {
        movement.x += 1.0;
    }
    if is_key_down(KeyCode::W) {
        movement.z += 1.0;
    }
    if is_key_down(KeyCode::S) {
        movement.z -= 1.0;
    }

    // When moving diagonally (e.g. W + D), the combined vector length is sqrt(1^2 + 1^2) ≈ 1.414.
    // Without normalization, diagonal movement would be 41% faster than moving in a straight line.
    let length_squared = movement.length_squared();
    if length_squared > 1.0 {
        let inverse_length = length_squared.sqrt().recip();
        movement = movement.scale(inverse_length);
    }

    // Convert screen-relative WASD input into world-space coordinates based on camera yaw.
    // In `render.rs`, camera forward is (sin_yaw, cos_yaw) and right is (-cos_yaw, sin_yaw).
    // Pressing 'W' (+Z) moves along forward, and 'D' (+X) moves along right.
    let (sin_yaw, cos_yaw) = camera_yaw.sin_cos();
    let forward_x = sin_yaw;
    let forward_z = cos_yaw;
    let right_x = -cos_yaw;
    let right_z = sin_yaw;

    // Combine forward and right vectors scaled by the player's movement input.
    game_types::Vec3::new(
        movement.x * right_x + movement.z * forward_x,
        0.0,
        movement.x * right_z + movement.z * forward_z,
    )
}
