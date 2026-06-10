//! The **canonical nine-arena identity** — the one table both binaries read.
//!
//! Each [`Arena`] is a class's home in the 3×3 grid (index `0..9`, row-major) and
//! carries the two things every consumer needs to agree on: the [`ThemeId`] it wears
//! and its `(skybox, foreground)` atmosphere [`Voice`]s. The game ([`crate`]'s
//! `intro_scene`) renders the whole grid from [`Arena::ALL`]; the storybook renders
//! one arena at a time and layers its own swarm/boss/prop visuals on top — but both
//! read theme + voices from HERE, so the identity can never drift across the crates
//! (the failure the storybook's per-arena spec was built to kill, now lifted up a
//! level so it holds across binaries too).

use bevy::prelude::*;

use crate::atmosphere::{CloudStyle, Voice};
use crate::boss::{BossShell, BossSpec, CoreMesh, LightBehavior};
use crate::swarm::{Mote, SwarmMotion, SwarmSpec};
use crate::theme::{ThemeId, Tint};

/// One of the nine arenas (3×3 grid, row-major). The discriminant IS the grid index.
/// Also a component: the game tags each arena root with its `Arena`, so systems can
/// resolve an entity's arena (e.g. a hero's `ChildOf` parent) without a second id.
#[derive(Component, Clone, Copy, PartialEq, Eq, Debug)]
#[component(immutable)]
pub enum Arena {
    Hunter,
    Guildmaster,
    Cardinal,
    Forager,
    Warrior,
    Thief,
    Alchemist,
    Merchant,
    Bard,
}

impl Arena {
    /// All nine arenas in grid order — index `i` sits at `grid::arena_offset(i)`.
    pub const ALL: [Arena; 9] = [
        Arena::Hunter,
        Arena::Guildmaster,
        Arena::Cardinal,
        Arena::Forager,
        Arena::Warrior,
        Arena::Thief,
        Arena::Alchemist,
        Arena::Merchant,
        Arena::Bard,
    ];

    /// This arena's grid index (`0..9`).
    pub fn index(self) -> usize {
        self as usize
    }

    /// The arena at grid index `i`, or `None` if out of range.
    pub fn from_index(i: usize) -> Option<Arena> {
        Self::ALL.get(i).copied()
    }

    /// The arena's display name (the place, not the class that calls it home).
    pub fn name(self) -> &'static str {
        match self {
            Arena::Hunter => "Labyrinth",
            Arena::Guildmaster => "Guild House",
            Arena::Cardinal => "Sanctum",
            Arena::Forager => "Mountain",
            Arena::Warrior => "Bastion",
            Arena::Thief => "Pawnshop",
            Arena::Alchemist => "Crucible",
            Arena::Merchant => "Casino",
            Arena::Bard => "Gala",
        }
    }

    /// The arena's kebab-case slug — its directory name under
    /// `assets/encounters/` (see [`crate::encounter`]). Derived from [`Self::name`],
    /// pinned here so a display-name tweak can't silently orphan score files.
    pub fn slug(self) -> &'static str {
        match self {
            Arena::Hunter => "labyrinth",
            Arena::Guildmaster => "guild-house",
            Arena::Cardinal => "sanctum",
            Arena::Forager => "mountain",
            Arena::Warrior => "bastion",
            Arena::Thief => "pawnshop",
            Arena::Alchemist => "crucible",
            Arena::Merchant => "casino",
            Arena::Bard => "gala",
        }
    }

    /// The theme this arena wears.
    pub fn theme(self) -> ThemeId {
        match self {
            Arena::Hunter => ThemeId::TokyoNight,
            Arena::Guildmaster => ThemeId::Coffee,
            Arena::Cardinal => ThemeId::Luxury,
            Arena::Forager => ThemeId::Forest,
            Arena::Warrior => ThemeId::GruvboxDark,
            Arena::Thief => ThemeId::AyuDark,
            Arena::Alchemist => ThemeId::Abyss,
            Arena::Merchant => ThemeId::RosePine,
            Arena::Bard => ThemeId::Synthwave,
        }
    }

    /// The arena's `(skybox, foreground)` atmosphere voices. The two are different
    /// instruments — a continuous-FIELD sky paired with a contrasting NEAR foreground
    /// (they differ on effect type, drift direction, register, tempo).
    pub fn voices(self) -> (Voice, Voice) {
        use CloudStyle::*;
        // (style, scale, drift_x, drift_y, coverage, vignette, speed)
        let v = |style, scale, dx, dy, coverage, vignette, speed| Voice {
            style,
            scale,
            drift: Vec2::new(dx, dy),
            coverage,
            vignette,
            speed,
        };
        match self {
            Arena::Hunter => (
                v(Banded, 3.0, 0.030, 0.000, 0.55, 0.40, 0.45),
                v(Streaks, 5.0, 0.000, -0.060, 0.50, 0.0, 0.90),
            ),
            Arena::Guildmaster => (
                v(Hearth, 2.0, 0.008, 0.004, 0.58, 0.30, 0.40),
                v(Embers, 6.0, 0.000, 0.050, 0.50, 0.0, 0.50),
            ),
            Arena::Cardinal => (
                v(Billows, 2.5, 0.012, 0.006, 0.50, 0.35, 0.35),
                v(Grain, 9.0, 0.000, -0.050, 0.55, 0.0, 0.50),
            ),
            Arena::Forager => (
                v(Vertical, 2.5, 0.000, 0.050, 0.58, 0.40, 1.00),
                v(Grain, 7.0, 0.060, 0.000, 0.50, 0.0, 0.70),
            ),
            Arena::Warrior => (
                v(Vertical, 2.2, 0.000, -0.040, 0.62, 0.45, 1.00),
                v(Embers, 5.0, 0.000, 0.060, 0.50, 0.0, 1.40),
            ),
            Arena::Thief => (
                v(Banded, 2.5, 0.000, 0.025, 0.50, 0.45, 0.40),
                v(Sweep, 2.5, 0.030, 0.000, 0.50, 0.0, 0.50),
            ),
            Arena::Alchemist => (
                v(Billows, 2.5, -0.020, 0.015, 0.60, 0.40, 1.00),
                v(Ripple, 4.0, 0.000, 0.000, 0.50, 0.0, 0.80),
            ),
            Arena::Merchant => (
                v(Spiral, 2.2, 0.000, 0.000, 0.54, 0.45, 0.70),
                v(Streaks, 5.0, 0.040, -0.030, 0.50, 0.0, 0.90),
            ),
            Arena::Bard => (
                v(Billows, 3.0, 0.015, 0.010, 0.54, 0.40, 0.50),
                v(Pulse, 3.0, 0.010, 0.010, 0.54, 0.0, 1.50),
            ),
        }
    }

    /// The full per-arena spec: identity (theme + voices) plus the sky-swarm, boss,
    /// and three flora/fauna props. The ONE source both binaries spawn an arena from.
    pub fn spec(self) -> ArenaSpec {
        use BossShell::*;
        use Mote::{Bubble, Dart, Flake, Spark};
        use Prim::{Capsule, Cone, Cuboid, Cylinder, Sphere};
        use SwarmMotion::*;

        let prop = |prim, dx, dy, tint: Tint| PropSpec {
            prim,
            offset: Vec2::new(dx, dy),
            tint,
        };
        let (sky, fg) = self.voices();
        let (swarm, boss, props): (SwarmSpec, BossSpec, [PropSpec; 3]) = match self {
            Arena::Hunter => (
                SwarmSpec {
                    motion: Patrol,
                    mote: Dart,
                    count: 12,
                    scale: 0.11,
                },
                BossSpec::Hollow {
                    shell: Obelisk,
                    core: CoreMesh::Cuboid(0.06, 0.06, 1.6),
                    core_z: 1.0,
                    behavior: LightBehavior::ObeliskBlade,
                    color: |t| t.brand(),
                },
                [
                    prop(Cuboid(0.55, 0.22, 0.4), -6.6, -2.4, |t| t.surface_3()),
                    prop(Cylinder(0.07, 0.55), 6.8, -2.7, |t| t.primary),
                    prop(Capsule(0.13, 0.3), 6.2, 2.9, |t| t.text_muted()),
                ],
            ),
            Arena::Guildmaster => (
                SwarmSpec {
                    motion: HearthRise,
                    mote: Spark,
                    count: 14,
                    scale: 0.07,
                },
                BossSpec::Home {
                    color: |t| t.brand(),
                },
                [
                    prop(Cuboid(0.45, 0.3, 0.32), -6.6, -2.5, |t| t.primary),
                    prop(Cylinder(0.16, 0.3), 6.6, -2.6, |t| t.surface_3()),
                    prop(Capsule(0.05, 0.4), 6.4, 2.7, |t| t.warning),
                ],
            ),
            Arena::Cardinal => (
                SwarmSpec {
                    motion: PendulumFall,
                    mote: Flake,
                    count: 12,
                    scale: 0.10,
                },
                BossSpec::Hollow {
                    shell: TorusHalo,
                    core: CoreMesh::Cuboid(0.55, 0.55, 0.04),
                    core_z: 0.06,
                    behavior: LightBehavior::HaloFill,
                    color: |t| t.warning,
                },
                [
                    prop(Cone(0.22, 0.5), 6.8, -2.4, |t| t.primary),
                    prop(Cylinder(0.06, 0.55), -6.6, 2.6, |t| t.surface_3()),
                    prop(Sphere(0.16), -5.9, -2.8, |t| t.warning),
                ],
            ),
            Arena::Forager => (
                SwarmSpec {
                    motion: GustDrift,
                    mote: Spark,
                    count: 16,
                    scale: 0.06,
                },
                BossSpec::Hollow {
                    shell: SteppedPyramid,
                    core: CoreMesh::Cuboid(0.1, 0.1, 0.9),
                    core_z: 0.5,
                    behavior: LightBehavior::ZigguratGrow,
                    color: |t| t.success,
                },
                [
                    prop(Sphere(0.28), -6.6, 3.0, |t| t.surface_3()),
                    prop(Sphere(0.17), 6.8, -2.6, |t| t.warning),
                    prop(Cone(0.1, 0.5), -5.8, -2.8, |t| t.accent),
                ],
            ),
            Arena::Warrior => (
                SwarmSpec {
                    motion: UpdraftChurn,
                    mote: Flake,
                    count: 13,
                    scale: 0.09,
                },
                BossSpec::Hollow {
                    shell: HexPrism,
                    core: CoreMesh::Sphere(0.2),
                    core_z: 0.22,
                    behavior: LightBehavior::PrismBlock,
                    color: |t| t.error,
                },
                [
                    prop(Cone(0.16, 0.55), 6.8, 2.4, |t| t.surface_3()),
                    prop(Sphere(0.16), -6.2, -2.9, |t| t.primary),
                    prop(Cylinder(0.3, 0.06), -6.6, 2.7, |t| t.warning),
                ],
            ),
            Arena::Thief => (
                SwarmSpec {
                    motion: FurtiveDart,
                    mote: Dart,
                    count: 10,
                    scale: 0.09,
                },
                BossSpec::Hollow {
                    shell: TriangularPrism,
                    core: CoreMesh::Cuboid(0.5, 0.12, 0.12),
                    core_z: 0.16,
                    behavior: LightBehavior::WedgeBeam,
                    color: |t| t.secondary,
                },
                [
                    prop(Sphere(0.28), 6.4, 2.9, |t| t.primary),
                    prop(Cone(0.16, 0.3), -6.7, -2.6, |t| t.surface_3()),
                    prop(Capsule(0.05, 0.5), -6.9, 2.1, |t| t.secondary),
                ],
            ),
            Arena::Alchemist => (
                SwarmSpec {
                    motion: OpposingDrift,
                    mote: Bubble,
                    count: 14,
                    scale: 0.08,
                },
                BossSpec::Hollow {
                    shell: TruncatedCone,
                    core: CoreMesh::Cuboid(0.5, 0.5, 0.06),
                    core_z: 0.12,
                    behavior: LightBehavior::CauldronRise,
                    color: |t| t.success,
                },
                [
                    prop(Cylinder(0.3, 0.06), -6.8, 2.1, |t| t.primary),
                    prop(Sphere(0.16), 6.2, -2.7, |t| t.surface_3()),
                    prop(Cylinder(0.05, 0.5), 6.6, 2.9, |t| t.secondary),
                ],
            ),
            Arena::Merchant => (
                SwarmSpec {
                    motion: TumbleArc,
                    mote: Flake,
                    count: 12,
                    scale: 0.10,
                },
                BossSpec::Hollow {
                    shell: HollowIcosphere,
                    core: CoreMesh::Sphere(0.22),
                    core_z: 0.3,
                    behavior: LightBehavior::GeodeShimmer,
                    color: |t| t.warning,
                },
                [
                    prop(Sphere(0.28), 6.6, -2.4, |t| t.warning),
                    prop(Cylinder(0.11, 0.55), -6.4, 2.9, |t| t.surface_3()),
                    prop(Cuboid(0.22, 0.22, 0.22), -0.4, -3.1, |t| t.accent),
                ],
            ),
            Arena::Bard => (
                SwarmSpec {
                    motion: BeatDrift,
                    mote: Flake,
                    count: 16,
                    scale: 0.09,
                },
                BossSpec::Hollow {
                    shell: CapsuleResonator,
                    core: CoreMesh::Cuboid(0.7, 0.05, 0.05),
                    core_z: 0.16,
                    behavior: LightBehavior::CapsulePulse,
                    color: |t| t.accent,
                },
                [
                    prop(Cuboid(0.42, 0.28, 0.06), -6.2, 2.9, |t| t.primary),
                    prop(Cylinder(0.26, 0.12), 6.6, -2.6, |t| t.secondary),
                    prop(Capsule(0.09, 0.3), -6.8, -2.2, |t| t.surface_3()),
                ],
            ),
        };
        ArenaSpec {
            theme: self.theme(),
            sky,
            fg,
            swarm,
            boss,
            props,
        }
    }
}

/// A low-poly placeholder primitive for one flora/fauna prop.
#[derive(Clone, Copy)]
pub enum Prim {
    Cuboid(f32, f32, f32),
    Sphere(f32),
    Cylinder(f32, f32),
    Cone(f32, f32),
    Capsule(f32, f32),
}

impl Prim {
    /// The mesh for this primitive, plus `(z_rest, stand)`: the resting height of its
    /// centre above the board, and whether it must be stood up along `+Z` (the
    /// upright forms — cylinder/cone/capsule — are authored along `+Y`).
    pub fn to_mesh(self) -> (Mesh, f32, bool) {
        match self {
            Prim::Cuboid(x, y, h) => (Cuboid::new(x, y, h).into(), h * 0.5, false),
            Prim::Sphere(r) => (Sphere::new(r).into(), r, false),
            Prim::Cylinder(r, h) => (Cylinder::new(r, h).into(), h * 0.5, true),
            Prim::Cone(r, h) => (
                Cone {
                    radius: r,
                    height: h,
                }
                .into(),
                h * 0.5,
                true,
            ),
            Prim::Capsule(r, h) => (Capsule3d::new(r, h).into(), h * 0.5 + r, true),
        }
    }
}

/// One ambient flora/fauna prop: shape, board-relative offset, on-theme tint.
#[derive(Clone, Copy)]
pub struct PropSpec {
    pub prim: Prim,
    pub offset: Vec2,
    pub tint: Tint,
}

/// Everything that makes one arena itself — its identity (theme + voices) plus the
/// six-layer content (sky-swarm, boss, props). Built by [`Arena::spec`].
#[derive(Clone, Copy)]
pub struct ArenaSpec {
    pub theme: ThemeId,
    pub sky: Voice,
    pub fg: Voice,
    pub swarm: SwarmSpec,
    pub boss: BossSpec,
    pub props: [PropSpec; 3],
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn index_round_trips_through_all() {
        for (i, &arena) in Arena::ALL.iter().enumerate() {
            assert_eq!(arena.index(), i);
            assert_eq!(Arena::from_index(i), Some(arena));
        }
        assert_eq!(Arena::from_index(9), None);
    }

    #[test]
    fn slugs_match_names_and_are_path_safe() {
        for arena in Arena::ALL {
            let expected = arena.name().to_lowercase().replace(' ', "-");
            assert_eq!(arena.slug(), expected, "slug drifted from name");
            assert!(
                arena
                    .slug()
                    .chars()
                    .all(|c| c.is_ascii_lowercase() || c == '-'),
                "slug {} is not path-safe",
                arena.slug()
            );
        }
    }

    #[test]
    fn arena_themes_are_exactly_the_game_set() {
        // The nine arena themes ARE the library's GAME set — one source, pinned here.
        let mut from_arena: Vec<ThemeId> = Arena::ALL.iter().map(|a| a.theme()).collect();
        from_arena.sort_by_key(|t| *t as usize);
        let mut game = ThemeId::GAME.to_vec();
        game.sort_by_key(|t| *t as usize);
        assert_eq!(from_arena, game, "Arena themes drifted from ThemeId::GAME");
    }
}
