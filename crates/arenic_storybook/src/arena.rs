//! The **single source of truth** for each arena's identity.
//!
//! One [`ArenaSpec`] row per arena binds together everything that used to live in
//! five separate `match StoryId` tables scattered across the crate — the theme,
//! the two atmosphere voices, the sky-swarm, the boss, and the flora/fauna props.
//! Every consumer ([`crate::foreground`], [`crate::stage`], [`crate::stories`])
//! reads its slice off the one [`spec`], so adding an arena (or re-ordering the
//! 3×3 grid) is a single compiler-checked row instead of five edits that can
//! silently drift apart. This mirrors the `Arena` aggregate in
//! `_docs/arena_model.go`.

use arenic_game::theme::{Theme, ThemeId};
use bevy::prelude::*;

use crate::foreground::{Mote, SwarmMotion};
use crate::hollow::LightBehavior;
use crate::stage::Prim;
use crate::stories::StoryId;

/// A theme token (a function of the active [`Theme`]) — the colour a piece follows.
pub(crate) type Tint = fn(&Theme) -> Color;

/// The atmosphere cloud-shader styles, in the order the WGSL decodes them.
///
/// The discriminants ARE the `flags.x` values `assets/shaders/arena_fog.wgsl`
/// branches on (`style < 0.5` → `Banded`, …) — keep the two in lockstep.
#[derive(Clone, Copy, PartialEq, Eq, Debug)]
#[repr(u32)]
pub(crate) enum CloudStyle {
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
    /// The value packed into the shader's `flags.x`.
    pub(crate) fn as_f32(self) -> f32 {
        self as u32 as f32
    }
}

/// One atmosphere voice (one shader plane): which effect, how big, which way it
/// drifts, how dense, how strong its vignette, how fast. Named fields so a designer
/// edits `coverage` by name, never by counting commas in a 7-float tuple.
#[derive(Clone, Copy)]
pub(crate) struct Voice {
    pub style: CloudStyle,
    pub scale: f32,
    pub drift: Vec2,
    pub coverage: f32,
    pub vignette: f32,
    pub speed: f32,
}

/// The sky-swarm for an arena: its drift archetype, mote silhouette, count, size.
#[derive(Clone, Copy)]
pub(crate) struct SwarmSpec {
    pub motion: SwarmMotion,
    pub mote: Mote,
    pub count: usize,
    pub scale: f32,
}

/// The emissive inner-core primitive of a hollow boss.
#[derive(Clone, Copy)]
pub(crate) enum CoreMesh {
    Cuboid(f32, f32, f32),
    Sphere(f32),
}

impl CoreMesh {
    pub(crate) fn to_mesh(self) -> Mesh {
        match self {
            CoreMesh::Cuboid(x, y, z) => Cuboid::new(x, y, z).into(),
            CoreMesh::Sphere(r) => Sphere::new(r).into(),
        }
    }
}

/// Which dark glTF shell a hollow boss wears (mapped to its loader in
/// [`crate::stage`]). One name per `arenic_game::boss` shell.
#[derive(Clone, Copy)]
pub(crate) enum BossShell {
    Obelisk,
    TruncatedCone,
    TorusHalo,
    HexPrism,
    TriangularPrism,
    CapsuleResonator,
    SteppedPyramid,
    HollowIcosphere,
}

/// What sits at the centre of an arena.
#[derive(Clone, Copy)]
pub(crate) enum BossSpec {
    /// A hollow-light boss: a dark shell + an animated emissive core.
    Hollow {
        shell: BossShell,
        core: CoreMesh,
        core_z: f32,
        behavior: LightBehavior,
        color: Tint,
    },
    /// The Guild House home: a calm pyramid, no core (the safe hearth).
    Home { color: Tint },
}

/// One ambient flora/fauna prop: shape, board-relative offset, on-theme tint.
#[derive(Clone, Copy)]
pub(crate) struct PropSpec {
    pub prim: Prim,
    pub offset: Vec2,
    pub tint: Tint,
}

/// Everything that makes one arena itself — the single row.
#[derive(Clone, Copy)]
pub(crate) struct ArenaSpec {
    pub theme: ThemeId,
    pub sky: Voice,
    pub fg: Voice,
    pub swarm: SwarmSpec,
    pub boss: BossSpec,
    pub props: [PropSpec; 3],
}

/// The atmosphere for non-arena stories (the design-token pages): a calm,
/// generic sky + near haze so the style guide still has depth.
pub(crate) const DEFAULT_SKY: Voice = Voice {
    style: CloudStyle::Billows,
    scale: 2.5,
    drift: Vec2::new(0.010, 0.010),
    coverage: 0.50,
    vignette: 0.35,
    speed: 0.60,
};
pub(crate) const DEFAULT_FG: Voice = Voice {
    style: CloudStyle::Grain,
    scale: 7.0,
    drift: Vec2::new(-0.010, 0.000),
    coverage: 0.40,
    vignette: 0.0,
    speed: 0.80,
};

/// The one per-arena table. `None` for the non-arena (design-token) stories.
///
/// Each row pairs a SKYBOX continuous-FIELD voice with a contrasting FOREGROUND
/// NEAR voice (the two-voice harmony: they differ on effect type, drift direction,
/// register and tempo). The trailing comments name the intent, not the numbers.
pub(crate) fn spec(story: StoryId) -> Option<ArenaSpec> {
    use BossShell::*;
    use CloudStyle::*;
    use Mote::{Bubble, Dart, Flake, Spark};
    use Prim::{Capsule, Cone, Cuboid, Cylinder, Sphere};
    use SwarmMotion::*;

    // Voices are written as full named-field literals (a designer edits `coverage`
    // by name, never by counting commas). Props keep a tiny constructor since their
    // only positional values are an obvious x/y board offset.
    let prop = |prim, dx, dy, tint: Tint| PropSpec {
        prim,
        offset: Vec2::new(dx, dy),
        tint,
    };

    Some(match story {
        StoryId::Hunter => ArenaSpec {
            theme: ThemeId::TokyoNight,
            sky: Voice {
                style: Banded,
                scale: 3.0,
                drift: Vec2::new(0.030, 0.000),
                coverage: 0.55,
                vignette: 0.40,
                speed: 0.45,
            }, // bands drifting sideways
            fg: Voice {
                style: Streaks,
                scale: 5.0,
                drift: Vec2::new(0.000, -0.060),
                coverage: 0.50,
                vignette: 0.0,
                speed: 0.90,
            }, // fine streaks falling (perp)
            swarm: SwarmSpec {
                motion: Patrol,
                mote: Dart,
                count: 12,
                scale: 0.11,
            },
            boss: BossSpec::Hollow {
                shell: Obelisk,
                core: CoreMesh::Cuboid(0.06, 0.06, 1.6),
                core_z: 1.0,
                behavior: LightBehavior::ObeliskBlade,
                color: |t| t.brand(),
            },
            props: [
                prop(Cuboid(0.55, 0.22, 0.4), -6.6, -2.4, |t| t.surface_3()),
                prop(Cylinder(0.07, 0.55), 6.8, -2.7, |t| t.primary),
                prop(Capsule(0.13, 0.3), 6.2, 2.9, |t| t.text_muted()),
            ],
        },
        StoryId::Guildmaster => ArenaSpec {
            theme: ThemeId::Coffee,
            sky: Voice {
                style: Hearth,
                scale: 2.0,
                drift: Vec2::new(0.008, 0.004),
                coverage: 0.58,
                vignette: 0.30,
                speed: 0.40,
            }, // hearth haze breathing
            fg: Voice {
                style: Embers,
                scale: 6.0,
                drift: Vec2::new(0.000, 0.050),
                coverage: 0.50,
                vignette: 0.0,
                speed: 0.50,
            }, // warm motes rising
            swarm: SwarmSpec {
                motion: HearthRise,
                mote: Spark,
                count: 14,
                scale: 0.07,
            },
            boss: BossSpec::Home {
                color: |t| t.brand(),
            },
            props: [
                // The home's warm hearth, a barrel, and a lantern post (safe + cosy).
                prop(Cuboid(0.45, 0.3, 0.32), -6.6, -2.5, |t| t.primary),
                prop(Cylinder(0.16, 0.3), 6.6, -2.6, |t| t.surface_3()),
                prop(Capsule(0.05, 0.4), 6.4, 2.7, |t| t.warning),
            ],
        },
        StoryId::Cardinal => ArenaSpec {
            theme: ThemeId::Luxury,
            sky: Voice {
                style: Billows,
                scale: 2.5,
                drift: Vec2::new(0.012, 0.006),
                coverage: 0.50,
                vignette: 0.35,
                speed: 0.35,
            }, // gold incense rolling
            fg: Voice {
                style: Grain,
                scale: 9.0,
                drift: Vec2::new(0.000, -0.050),
                coverage: 0.55,
                vignette: 0.0,
                speed: 0.50,
            }, // fine gilt grain sifting down
            swarm: SwarmSpec {
                motion: PendulumFall,
                mote: Flake,
                count: 12,
                scale: 0.10,
            },
            boss: BossSpec::Hollow {
                shell: TorusHalo,
                core: CoreMesh::Cuboid(0.55, 0.55, 0.04),
                core_z: 0.06,
                behavior: LightBehavior::HaloFill,
                color: |t| t.warning,
            },
            props: [
                prop(Cone(0.22, 0.5), 6.8, -2.4, |t| t.primary),
                prop(Cylinder(0.06, 0.55), -6.6, 2.6, |t| t.surface_3()),
                prop(Sphere(0.16), -5.9, -2.8, |t| t.warning),
            ],
        },
        StoryId::Forager => ArenaSpec {
            theme: ThemeId::Forest,
            sky: Voice {
                style: Vertical,
                scale: 2.5,
                drift: Vec2::new(0.000, 0.050),
                coverage: 0.58,
                vignette: 0.40,
                speed: 1.00,
            }, // green spores rising
            fg: Voice {
                style: Grain,
                scale: 7.0,
                drift: Vec2::new(0.060, 0.000),
                coverage: 0.50,
                vignette: 0.0,
                speed: 0.70,
            }, // wind-grain drifting (perp)
            swarm: SwarmSpec {
                motion: GustDrift,
                mote: Spark,
                count: 16,
                scale: 0.06,
            },
            boss: BossSpec::Hollow {
                shell: SteppedPyramid,
                core: CoreMesh::Cuboid(0.1, 0.1, 0.9),
                core_z: 0.5,
                behavior: LightBehavior::ZigguratGrow,
                color: |t| t.success,
            },
            props: [
                prop(Sphere(0.28), -6.6, 3.0, |t| t.surface_3()),
                prop(Sphere(0.17), 6.8, -2.6, |t| t.warning),
                prop(Cone(0.1, 0.5), -5.8, -2.8, |t| t.accent),
            ],
        },
        StoryId::Warrior => ArenaSpec {
            theme: ThemeId::GruvboxDark,
            sky: Voice {
                style: Vertical,
                scale: 2.2,
                drift: Vec2::new(0.000, -0.040),
                coverage: 0.62,
                vignette: 0.45,
                speed: 1.00,
            }, // ash lid pressing down
            fg: Voice {
                style: Embers,
                scale: 5.0,
                drift: Vec2::new(0.000, 0.060),
                coverage: 0.50,
                vignette: 0.0,
                speed: 1.40,
            }, // sparks rising fast (opposite)
            swarm: SwarmSpec {
                motion: UpdraftChurn,
                mote: Flake,
                count: 13,
                scale: 0.09,
            },
            boss: BossSpec::Hollow {
                shell: HexPrism,
                core: CoreMesh::Sphere(0.2),
                core_z: 0.22,
                behavior: LightBehavior::PrismBlock,
                color: |t| t.error,
            },
            props: [
                prop(Cone(0.16, 0.55), 6.8, 2.4, |t| t.surface_3()),
                prop(Sphere(0.16), -6.2, -2.9, |t| t.primary),
                prop(Cylinder(0.3, 0.06), -6.6, 2.7, |t| t.warning),
            ],
        },
        StoryId::Thief => ArenaSpec {
            theme: ThemeId::AyuDark,
            sky: Voice {
                style: Banded,
                scale: 2.5,
                drift: Vec2::new(0.000, 0.025),
                coverage: 0.50,
                vignette: 0.45,
                speed: 0.40,
            }, // cyan bands, vertical drift
            fg: Voice {
                style: Sweep,
                scale: 2.5,
                drift: Vec2::new(0.030, 0.000),
                coverage: 0.50,
                vignette: 0.0,
                speed: 0.50,
            }, // watch-band sweeping (perp)
            swarm: SwarmSpec {
                motion: FurtiveDart,
                mote: Dart,
                count: 10,
                scale: 0.09,
            },
            boss: BossSpec::Hollow {
                shell: TriangularPrism,
                core: CoreMesh::Cuboid(0.5, 0.12, 0.12),
                core_z: 0.16,
                behavior: LightBehavior::WedgeBeam,
                color: |t| t.secondary,
            },
            props: [
                prop(Sphere(0.28), 6.4, 2.9, |t| t.primary),
                prop(Cone(0.16, 0.3), -6.7, -2.6, |t| t.surface_3()),
                prop(Capsule(0.05, 0.5), -6.9, 2.1, |t| t.secondary),
            ],
        },
        StoryId::Alchemist => ArenaSpec {
            theme: ThemeId::Abyss,
            sky: Voice {
                style: Billows,
                scale: 2.5,
                drift: Vec2::new(-0.020, 0.015),
                coverage: 0.60,
                vignette: 0.40,
                speed: 1.00,
            }, // lime smog rolling
            fg: Voice {
                style: Ripple,
                scale: 4.0,
                drift: Vec2::new(0.000, 0.000),
                coverage: 0.50,
                vignette: 0.0,
                speed: 0.80,
            }, // bubble-ripples (radial)
            swarm: SwarmSpec {
                motion: OpposingDrift,
                mote: Bubble,
                count: 14,
                scale: 0.08,
            },
            boss: BossSpec::Hollow {
                shell: TruncatedCone,
                core: CoreMesh::Cuboid(0.5, 0.5, 0.06),
                core_z: 0.12,
                behavior: LightBehavior::CauldronRise,
                color: |t| t.success,
            },
            props: [
                prop(Cylinder(0.3, 0.06), -6.8, 2.1, |t| t.primary),
                prop(Sphere(0.16), 6.2, -2.7, |t| t.surface_3()),
                prop(Cylinder(0.05, 0.5), 6.6, 2.9, |t| t.secondary),
            ],
        },
        StoryId::Merchant => ArenaSpec {
            theme: ThemeId::RosePine,
            sky: Voice {
                style: Spiral,
                scale: 2.2,
                drift: Vec2::new(0.000, 0.000),
                coverage: 0.54,
                vignette: 0.45,
                speed: 0.70,
            }, // gold-plum smoke swirling
            fg: Voice {
                style: Streaks,
                scale: 5.0,
                drift: Vec2::new(0.040, -0.030),
                coverage: 0.50,
                vignette: 0.0,
                speed: 0.90,
            }, // coin-streaks tumbling
            swarm: SwarmSpec {
                motion: TumbleArc,
                mote: Flake,
                count: 12,
                scale: 0.10,
            },
            boss: BossSpec::Hollow {
                shell: HollowIcosphere,
                core: CoreMesh::Sphere(0.22),
                core_z: 0.3,
                behavior: LightBehavior::GeodeShimmer,
                color: |t| t.warning,
            },
            props: [
                prop(Sphere(0.28), 6.6, -2.4, |t| t.warning),
                prop(Cylinder(0.11, 0.55), -6.4, 2.9, |t| t.surface_3()),
                prop(Cuboid(0.22, 0.22, 0.22), -0.4, -3.1, |t| t.accent),
            ],
        },
        StoryId::Bard => ArenaSpec {
            theme: ThemeId::Synthwave,
            sky: Voice {
                style: Billows,
                scale: 3.0,
                drift: Vec2::new(0.015, 0.010),
                coverage: 0.54,
                vignette: 0.40,
                speed: 0.50,
            }, // violet haze drifting
            fg: Voice {
                style: Pulse,
                scale: 3.0,
                drift: Vec2::new(0.010, 0.010),
                coverage: 0.54,
                vignette: 0.0,
                speed: 1.50,
            }, // throb on the beat (rhythm)
            swarm: SwarmSpec {
                motion: BeatDrift,
                mote: Flake,
                count: 16,
                scale: 0.09,
            },
            boss: BossSpec::Hollow {
                shell: CapsuleResonator,
                core: CoreMesh::Cuboid(0.7, 0.05, 0.05),
                core_z: 0.16,
                behavior: LightBehavior::CapsulePulse,
                color: |t| t.accent,
            },
            props: [
                prop(Cuboid(0.42, 0.28, 0.06), -6.2, 2.9, |t| t.primary),
                prop(Cylinder(0.26, 0.12), 6.6, -2.6, |t| t.secondary),
                prop(Capsule(0.09, 0.3), -6.8, -2.2, |t| t.surface_3()),
            ],
        },
        _ => return None,
    })
}

#[cfg(test)]
mod tests {
    use super::*;
    use arenic_game::theme::ThemeId;

    /// The nine arena stories (the 3×3 grid).
    const ARENA_STORIES: [StoryId; 9] = [
        StoryId::Hunter,
        StoryId::Guildmaster,
        StoryId::Cardinal,
        StoryId::Forager,
        StoryId::Warrior,
        StoryId::Thief,
        StoryId::Alchemist,
        StoryId::Merchant,
        StoryId::Bard,
    ];

    #[test]
    fn every_arena_has_a_spec() {
        for (i, s) in ARENA_STORIES.into_iter().enumerate() {
            assert!(
                spec(s).is_some(),
                "arena story #{i} is missing an ArenaSpec"
            );
        }
    }

    #[test]
    fn non_arena_story_has_no_spec() {
        assert!(spec(StoryId::Colors).is_none());
    }

    #[test]
    fn arena_themes_are_exactly_the_game_set() {
        // The per-arena themes in the spec must be exactly the library's GAME set —
        // two parallel tables in different crates, pinned equal here.
        let mut from_spec: Vec<ThemeId> = ARENA_STORIES
            .iter()
            .map(|&s| spec(s).unwrap().theme)
            .collect();
        from_spec.sort_by_key(|t| *t as usize);
        let mut game = ThemeId::GAME.to_vec();
        game.sort_by_key(|t| *t as usize);
        assert_eq!(
            from_spec, game,
            "ArenaSpec themes drifted from ThemeId::GAME"
        );
    }

    #[test]
    fn cloud_style_packs_to_its_discriminant() {
        // The WGSL shader branches on these exact f32 values.
        assert_eq!(CloudStyle::Banded.as_f32(), 0.0);
        assert_eq!(CloudStyle::Hearth.as_f32(), 7.0);
        assert_eq!(CloudStyle::Ripple.as_f32(), 10.0);
    }
}
