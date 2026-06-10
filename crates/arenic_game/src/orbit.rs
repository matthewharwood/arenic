//! Reusable **orbit / pan / zoom** camera controls for inspecting a 3D scene —
//! a lightweight Blender-style viewport gizmo.
//!
//! Attach an [`OrbitCamera`] to a `Camera3d` and add [`OrbitCameraPlugin`]. The
//! camera starts **locked** at the pose you pass to [`OrbitCamera::new`] (its
//! "home"); insert [`OrbitUnlocked`] (e.g. from a UI button) to drive it with the
//! mouse/trackpad, and remove it + call [`OrbitCamera::reset`] to snap back home.
//!
//! Controls (Blender-like):
//!
//! | Action | Trackpad | Mouse |
//! | --- | --- | --- |
//! | Orbit | two-finger drag | left-drag |
//! | Zoom  | pinch | wheel |
//! | Pan   | Shift + two-finger | Shift + left-drag |
//!
//! Trackpad two-finger drag and the mouse wheel both arrive as `MouseWheel`, but
//! the `MouseScrollUnit` tells them apart (`Pixel` = trackpad → orbit, `Line` =
//! wheel → zoom), so the two never collide.

use bevy::input::gestures::PinchGesture;
use bevy::input::mouse::{MouseMotion, MouseScrollUnit, MouseWheel};
use bevy::prelude::*;

/// A camera pose in orbit terms: a `focus` point, `yaw`/`pitch` around it, and
/// the `radius` (distance / zoom).
#[derive(Clone, Copy)]
pub struct OrbitPose {
    pub focus: Vec3,
    pub yaw: f32,
    pub pitch: f32,
    pub radius: f32,
}

/// Orbit state for a `Camera3d`. The live pose is the public fields; `home` is
/// where [`reset`](Self::reset) returns to.
#[derive(Component)]
pub struct OrbitCamera {
    pub focus: Vec3,
    pub yaw: f32,
    pub pitch: f32,
    pub radius: f32,
    pub home: OrbitPose,
}

/// Present while the camera follows mouse/trackpad input; absent = locked, the
/// camera holds its pose.
#[derive(Component)]
#[component(immutable)]
pub struct OrbitUnlocked;

impl OrbitCamera {
    /// Builds a locked orbit camera whose home (and starting) pose looks at
    /// `focus` from `radius` away, at the given `yaw`/`pitch` (radians). A pose
    /// of `yaw = pitch = 0` looks straight down `−Z` with `+Y` up.
    pub fn new(focus: Vec3, yaw: f32, pitch: f32, radius: f32) -> Self {
        Self {
            focus,
            yaw,
            pitch,
            radius,
            home: OrbitPose {
                focus,
                yaw,
                pitch,
                radius,
            },
        }
    }

    /// Snaps the live pose back to `home`.
    pub fn reset(&mut self) {
        self.focus = self.home.focus;
        self.yaw = self.home.yaw;
        self.pitch = self.home.pitch;
        self.radius = self.home.radius;
    }

    /// The world transform for the current pose.
    pub fn transform(&self) -> Transform {
        let rotation = Quat::from_rotation_y(self.yaw) * Quat::from_rotation_x(self.pitch);
        let eye = self.focus + rotation * Vec3::new(0.0, 0.0, self.radius);
        Transform::from_translation(eye).looking_at(self.focus, rotation * Vec3::Y)
    }
}

pub struct OrbitCameraPlugin;

impl Plugin for OrbitCameraPlugin {
    fn build(&self, app: &mut App) {
        app.add_systems(Update, (orbit_input, apply_orbit).chain());
    }
}

const ORBIT_SPEED: f32 = 0.005; // radians per pixel
const PAN_SPEED: f32 = 0.0016; // focus units per pixel, scaled by radius
const ZOOM_WHEEL: f32 = 0.12; // per wheel line
const ZOOM_PINCH: f32 = 2.0; // per pinch unit
const PITCH_LIMIT: f32 = 1.54; // ~88°
const MIN_RADIUS: f32 = 0.4;
const MAX_RADIUS: f32 = 140.0;

/// Reads mouse/trackpad input and updates every unlocked [`OrbitCamera`].
fn orbit_input(
    mut cameras: Query<&mut OrbitCamera, With<OrbitUnlocked>>,
    buttons: Res<ButtonInput<MouseButton>>,
    keys: Res<ButtonInput<KeyCode>>,
    mut motion: MessageReader<MouseMotion>,
    mut wheel: MessageReader<MouseWheel>,
    mut pinch: MessageReader<PinchGesture>,
) {
    let shift = keys.pressed(KeyCode::ShiftLeft) || keys.pressed(KeyCode::ShiftRight);
    let left = buttons.pressed(MouseButton::Left);

    let mut orbit = Vec2::ZERO; // accumulated orbit delta (px)
    let mut pan = Vec2::ZERO; // accumulated pan delta (px)
    let mut zoom = 1.0_f32; // multiplicative radius change

    // Mouse drag (relative motion while the left button is held).
    let mut drag = Vec2::ZERO;
    for ev in motion.read() {
        drag += ev.delta;
    }
    if left {
        if shift {
            pan += drag;
        } else {
            orbit += drag;
        }
    }

    // Wheel: trackpad two-finger (Pixel) orbits/pans; real wheel (Line) zooms.
    for ev in wheel.read() {
        match ev.unit {
            MouseScrollUnit::Line => zoom *= (1.0 - ev.y * ZOOM_WHEEL).max(0.1),
            MouseScrollUnit::Pixel => {
                let d = Vec2::new(ev.x, -ev.y);
                if shift {
                    pan += d;
                } else {
                    orbit += d;
                }
            }
        }
    }

    // Trackpad pinch zooms.
    for ev in pinch.read() {
        zoom *= (1.0 - ev.0 * ZOOM_PINCH).max(0.1);
    }

    let idle = orbit == Vec2::ZERO && pan == Vec2::ZERO && (zoom - 1.0).abs() < f32::EPSILON;
    if idle {
        return;
    }

    for mut cam in &mut cameras {
        cam.yaw -= orbit.x * ORBIT_SPEED;
        cam.pitch = (cam.pitch - orbit.y * ORBIT_SPEED).clamp(-PITCH_LIMIT, PITCH_LIMIT);
        cam.radius = (cam.radius * zoom).clamp(MIN_RADIUS, MAX_RADIUS);
        if pan != Vec2::ZERO {
            let t = cam.transform();
            let scale = PAN_SPEED * cam.radius;
            cam.focus += (Vec3::from(t.right()) * -pan.x + Vec3::from(t.up()) * pan.y) * scale;
        }
    }
}

/// Writes each [`OrbitCamera`]'s pose to its `Transform` every frame, so a
/// locked camera holds home and an unlocked one follows the input.
fn apply_orbit(mut cameras: Query<(&OrbitCamera, &mut Transform)>) {
    for (cam, mut transform) in &mut cameras {
        transform.set_if_neq(cam.transform());
    }
}
