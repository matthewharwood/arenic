//! Per-arena **atmosphere** — the cloud/fog shader on two world-space planes per
//! arena: an opaque **skybox** behind the board and a blended **foreground** haze
//! in front. Each arena pairs a continuous-FIELD skybox voice with a contrasting
//! NEAR foreground voice; both re-tone from the arena's theme. Drawn by
//! `assets/shaders/arena_fog.wgsl`; add [`AtmospherePlugin`].

use bevy::pbr::{Material, MaterialPlugin};
use bevy::prelude::*;
use bevy::render::render_resource::{AsBindGroup, ShaderType};
use bevy::shader::ShaderRef;

use crate::theme::Theme;

const SHADER: &str = "shaders/arena_fog.wgsl";

/// The cloud-shader styles, in the order the WGSL decodes them (the discriminant
/// IS the `flags.x` value the shader branches on — keep the two in lockstep).
#[derive(Clone, Copy, PartialEq, Eq, Debug)]
#[repr(u32)]
pub enum CloudStyle {
    Banded = 0,
    Billows = 1,
    Vertical = 2,
    Spiral = 3,
    Embers = 4,
    Sweep = 5,
    Pulse = 6,
    Hearth = 7,
    Grain = 8,
    Streaks = 9,
    Ripple = 10,
}

impl CloudStyle {
    /// The value packed into the shader's `flags.x` (both binaries pack the same way).
    pub fn as_f32(self) -> f32 {
        self as u32 as f32
    }
}

/// One atmosphere voice (one plane): which effect, how big, which way it drifts,
/// how dense, how strong its vignette, how fast.
#[derive(Clone, Copy)]
pub struct Voice {
    pub style: CloudStyle,
    pub scale: f32,
    pub drift: Vec2,
    pub coverage: f32,
    pub vignette: f32,
    pub speed: f32,
}

/// The shader's per-plane uniform. Matches the `CloudParams` struct in the WGSL.
#[derive(Clone, Copy, Default, ShaderType)]
pub struct CloudParams {
    tint: Vec4,
    accent: Vec4,
    vignette: Vec4,
    flags: Vec4,
    mode: Vec4,
}

/// The atmosphere cloud/fog material (one per plane).
#[derive(Asset, TypePath, AsBindGroup, Clone)]
pub struct CloudFog {
    #[uniform(0)]
    params: CloudParams,
}

impl Material for CloudFog {
    fn fragment_shader() -> ShaderRef {
        SHADER.into()
    }
    fn alpha_mode(&self) -> AlphaMode {
        // Skybox = opaque backdrop; foreground = blended near haze.
        if self.params.mode.x > 0.5 {
            AlphaMode::Blend
        } else {
            AlphaMode::Opaque
        }
    }
}

fn lin(c: Color, a: f32) -> Vec4 {
    let l = c.to_linear();
    Vec4::new(l.red, l.green, l.blue, a)
}

/// Which of the two atmosphere planes a material is built for.
#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub enum Plane {
    Skybox,
    Foreground,
}

/// Builds the shader material for one plane: [`Plane::Foreground`] picks the
/// blended, edge-framed near layer; [`Plane::Skybox`] the opaque full skybox.
pub fn cloud_material(voice: Voice, theme: &Theme, plane: Plane) -> CloudFog {
    CloudFog {
        params: CloudParams {
            tint: lin(theme.base_300, voice.coverage),
            accent: lin(theme.primary, voice.speed),
            vignette: lin(theme.base_300, voice.vignette),
            flags: Vec4::new(
                voice.style.as_f32(),
                voice.scale,
                voice.drift.x,
                voice.drift.y,
            ),
            mode: match plane {
                Plane::Foreground => Vec4::new(1.0, 0.5, 0.0, 0.0),
                Plane::Skybox => Vec4::new(0.0, 1.0, 0.0, 0.0),
            },
        },
    }
}

/// Registers the atmosphere material pipeline.
pub struct AtmospherePlugin;

impl Plugin for AtmospherePlugin {
    fn build(&self, app: &mut App) {
        app.add_plugins(MaterialPlugin::<CloudFog>::default());
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::theme::ThemeId;

    #[test]
    fn cloud_style_packs_to_its_discriminant() {
        // The WGSL shader branches on these exact f32 values — keep them in lockstep.
        assert_eq!(CloudStyle::Banded.as_f32(), 0.0);
        assert_eq!(CloudStyle::Hearth.as_f32(), 7.0);
        assert_eq!(CloudStyle::Ripple.as_f32(), 10.0);
    }

    #[test]
    fn mode_flags_distinguish_skybox_from_foreground() {
        // The shader's `mode.x` picks edge-framed vs full; `mode.y` scales alpha.
        let theme = ThemeId::Dark.palette();
        let voice = Voice {
            style: CloudStyle::Billows,
            scale: 2.5,
            drift: Vec2::ZERO,
            coverage: 0.5,
            vignette: 0.35,
            speed: 0.6,
        };
        assert_eq!(
            cloud_material(voice, &theme, Plane::Skybox).params.mode,
            Vec4::new(0.0, 1.0, 0.0, 0.0),
            "skybox is opaque/full",
        );
        assert_eq!(
            cloud_material(voice, &theme, Plane::Foreground).params.mode,
            Vec4::new(1.0, 0.5, 0.0, 0.0),
            "foreground is edge-framed/half-alpha",
        );
    }
}
