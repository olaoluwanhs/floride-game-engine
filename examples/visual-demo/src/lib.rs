//! Reusable 3D visual demonstration and debug rendering tools for Floride Game Engine.
//!
//! Provides a Macroquad-based 3D scene renderer, parametric camera system, and debug drawing utilities
//! for any game or simulation implementing [`game_api::GameAdapter`].

pub mod render;
pub mod scene;

pub use render::{
    draw_error, draw_hud, draw_scene, CameraSettings, GameCamera, OrbitCamera,
};
pub use scene::{DemoObject, ObjectKind, RayHitVisual, RayVisual, RenderItem, Scene};
