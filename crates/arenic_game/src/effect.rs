//! **Effect tracks** — non-destructive, keyframed, GPU-path layer effects
//! (`_docs/AUTHORING_UI.md` §4): the "smart objects" of the authoring suite.
//!
//! A [`Layer`](crate::layer::Layer) carries [`EffectTrack`]s (Scale, Opacity);
//! each track is tick-keyed [`EffectKey`]s eased with [`EaseKind`] — a
//! serde-stable local vocabulary mapped onto `bevy::math::curve::EaseFunction`
//! (bevy's enum is `#[non_exhaustive]` and its serde sits behind a non-default
//! feature; a local enum keeps the published file format ours).
//!
//! Application never touches authored data or shared assets (the CSS
//! no-repaint analogy):
//! - **Scale** writes `Transform.scale` on the layer's bound root —
//!   propagation-only cost, no asset churn.
//! - **Opacity** swaps descendants onto CLONE-ONCE material instances
//!   (`AlphaMode::Blend`) and mutates only the clones' `base_color` alpha;
//!   removing the track restores the original handles, pixel-perfect.
//!
//! Effects are PRESENTATION: they sample the arena clock (so playback and
//! scrubbing both preview live) and never feed record/replay state — exempt
//! from the strict-determinism sim doctrine (f32 easing is fine here).

use bevy::math::curve::{Curve, EaseFunction, EasingCurve, JumpAt};
use bevy::prelude::*;
use serde::{Deserialize, Serialize};

use crate::layer::{ArenaStack, LayerBinding};
use crate::timeline::ArenaClock;

/// The file-stable easing vocabulary. Map to bevy with [`EaseKind::function`];
/// extend by appending variants (never reorder — RON names are the contract).
#[derive(Clone, Copy, PartialEq, Eq, Debug, Default, Serialize, Deserialize)]
pub enum EaseKind {
    #[default]
    Linear,
    SineIn,
    SineOut,
    SineInOut,
    QuadIn,
    QuadOut,
    QuadInOut,
    CubicIn,
    CubicOut,
    CubicInOut,
    ExpoIn,
    ExpoOut,
    ExpoInOut,
    BackIn,
    BackOut,
    BackInOut,
    ElasticIn,
    ElasticOut,
    ElasticInOut,
    BounceIn,
    BounceOut,
    BounceInOut,
    SmoothStep,
    /// The value HOLDS until the next key (a step at the segment's end).
    Hold,
}

impl EaseKind {
    pub fn function(self) -> EaseFunction {
        match self {
            EaseKind::Linear => EaseFunction::Linear,
            EaseKind::SineIn => EaseFunction::SineIn,
            EaseKind::SineOut => EaseFunction::SineOut,
            EaseKind::SineInOut => EaseFunction::SineInOut,
            EaseKind::QuadIn => EaseFunction::QuadraticIn,
            EaseKind::QuadOut => EaseFunction::QuadraticOut,
            EaseKind::QuadInOut => EaseFunction::QuadraticInOut,
            EaseKind::CubicIn => EaseFunction::CubicIn,
            EaseKind::CubicOut => EaseFunction::CubicOut,
            EaseKind::CubicInOut => EaseFunction::CubicInOut,
            EaseKind::ExpoIn => EaseFunction::ExponentialIn,
            EaseKind::ExpoOut => EaseFunction::ExponentialOut,
            EaseKind::ExpoInOut => EaseFunction::ExponentialInOut,
            EaseKind::BackIn => EaseFunction::BackIn,
            EaseKind::BackOut => EaseFunction::BackOut,
            EaseKind::BackInOut => EaseFunction::BackInOut,
            EaseKind::ElasticIn => EaseFunction::ElasticIn,
            EaseKind::ElasticOut => EaseFunction::ElasticOut,
            EaseKind::ElasticInOut => EaseFunction::ElasticInOut,
            EaseKind::BounceIn => EaseFunction::BounceIn,
            EaseKind::BounceOut => EaseFunction::BounceOut,
            EaseKind::BounceInOut => EaseFunction::BounceInOut,
            EaseKind::SmoothStep => EaseFunction::SmoothStep,
            EaseKind::Hold => EaseFunction::Steps(1, JumpAt::End),
        }
    }
}

/// What an [`EffectTrack`] drives on the layer's bound entity.
#[derive(Clone, Copy, PartialEq, Eq, Debug, Serialize, Deserialize)]
pub enum EffectKind {
    /// Uniform `Transform.scale` (1.0 = authored size).
    Scale,
    /// Material alpha on the bound subtree (1.0 = authored look).
    Opacity,
}

/// One keyframe: at `tick` the track is exactly `value`; `ease` shapes the
/// segment FROM this key to the next (AE semantics).
#[derive(Clone, Copy, PartialEq, Debug, Serialize, Deserialize)]
pub struct EffectKey {
    pub tick: u32,
    pub value: f32,
    pub ease: EaseKind,
}

/// A keyframed effect on one layer. Keys stay tick-sorted (the editor inserts
/// in order; [`EffectTrack::sample`] assumes it).
#[derive(Clone, PartialEq, Debug, Serialize, Deserialize)]
pub struct EffectTrack {
    pub kind: EffectKind,
    pub keys: Vec<EffectKey>,
}

impl EffectTrack {
    /// The track's value at `tick`: clamped to the first/last key outside the
    /// keyed range, eased between adjacent keys inside it. `None` only when
    /// the track has no keys at all.
    pub fn sample(&self, tick: u32) -> Option<f32> {
        let first = self.keys.first()?;
        if tick <= first.tick {
            return Some(first.value);
        }
        let last = self
            .keys
            .last()
            .expect("invariant: non-empty checked via first()");
        if tick >= last.tick {
            return Some(last.value);
        }
        // First key strictly after `tick`; its predecessor starts the segment.
        let after = self.keys.partition_point(|key| key.tick <= tick);
        let (a, b) = (&self.keys[after.strict_sub(1)], &self.keys[after]);
        let span = b.tick.strict_sub(a.tick);
        let t = tick.strict_sub(a.tick) as f32 / span as f32;
        Some(EasingCurve::new(a.value, b.value, a.ease.function()).sample_clamped(t))
    }
}

/// Clone-once bookkeeping for an Opacity effect: which descendants were
/// swapped onto clone materials, their ORIGINAL handles (restored when the
/// track goes away), and each clone's authored base alpha.
#[derive(Component, Default)]
pub struct FadedMaterials {
    swapped: Vec<Swap>,
    last_opacity: f32,
}

struct Swap {
    descendant: Entity,
    original: Handle<StandardMaterial>,
    clone: Handle<StandardMaterial>,
    base_alpha: f32,
}

/// The Scale track: writes the bound root's `Transform.scale` (1.0 when no
/// track or no keys — removing the effect restores the authored size).
fn apply_scale_tracks(
    arenas: Query<(&ArenaClock, &ArenaStack)>,
    mut targets: Query<(&LayerBinding, &ChildOf, &mut Transform)>,
) {
    for (binding, child_of, mut transform) in &mut targets {
        let Ok((clock, arena_stack)) = arenas.get(child_of.parent()) else {
            continue;
        };
        let scale = arena_stack
            .stack
            .layer(binding.0)
            .and_then(|layer| {
                layer
                    .effects
                    .iter()
                    .find(|track| track.kind == EffectKind::Scale)
            })
            .and_then(|track| track.sample(clock.tick))
            .unwrap_or(1.0);
        // Write-if-changed: an unconditional write would re-dirty transform
        // propagation for every bound entity every frame.
        if (transform.scale.x - scale).abs() > 1e-4 {
            transform.scale = Vec3::splat(scale);
        }
    }
}

/// The Opacity track: lazily swaps the bound subtree onto clone materials
/// (`AlphaMode::Blend`) and drives only the clones' alpha; restores the
/// original handles when the track disappears.
fn apply_opacity_tracks(
    mut commands: Commands,
    arenas: Query<(&ArenaClock, &ArenaStack)>,
    mut targets: Query<(Entity, &LayerBinding, &ChildOf, Option<&mut FadedMaterials>)>,
    children: Query<&Children>,
    handles: Query<&MeshMaterial3d<StandardMaterial>>,
    mut materials: ResMut<Assets<StandardMaterial>>,
) {
    for (root, binding, child_of, faded) in &mut targets {
        let Ok((clock, arena_stack)) = arenas.get(child_of.parent()) else {
            continue;
        };
        let opacity = arena_stack
            .stack
            .layer(binding.0)
            .and_then(|layer| {
                layer
                    .effects
                    .iter()
                    .find(|track| track.kind == EffectKind::Opacity)
            })
            .and_then(|track| track.sample(clock.tick));

        match (opacity, faded) {
            (None, None) => {}
            (None, Some(faded)) => {
                // Track removed — restore the authored materials exactly.
                for swap in &faded.swapped {
                    commands
                        .entity(swap.descendant)
                        .insert(MeshMaterial3d(swap.original.clone()));
                }
                commands.entity(root).remove::<FadedMaterials>();
            }
            (Some(opacity), faded) => {
                let mut local = FadedMaterials::default();
                let (faded, deferred) = match faded {
                    Some(faded) => (faded.into_inner(), false),
                    None => (&mut local, true),
                };
                // Clone-once, incrementally: glTF scene materials appear a few
                // frames after spawn, so keep adopting new descendants.
                for descendant in children.iter_descendants(root) {
                    if faded.swapped.iter().any(|s| s.descendant == descendant) {
                        continue;
                    }
                    let Ok(handle) = handles.get(descendant) else {
                        continue;
                    };
                    let Some(original_material) = materials.get(&handle.0) else {
                        continue;
                    };
                    let mut clone = original_material.clone();
                    clone.alpha_mode = AlphaMode::Blend;
                    let base_alpha = clone.base_color.alpha();
                    let clone = materials.add(clone);
                    commands
                        .entity(descendant)
                        .insert(MeshMaterial3d(clone.clone()));
                    faded.swapped.push(Swap {
                        descendant,
                        original: handle.0.clone(),
                        clone,
                        base_alpha,
                    });
                    faded.last_opacity = f32::NAN; // force the alpha write below
                }
                if (faded.last_opacity - opacity).abs() > 1e-3 || faded.last_opacity.is_nan() {
                    for swap in &faded.swapped {
                        if let Some(material) = materials.get_mut(&swap.clone) {
                            material
                                .base_color
                                .set_alpha(swap.base_alpha * opacity.clamp(0.0, 1.0));
                        }
                    }
                    faded.last_opacity = opacity;
                }
                if deferred {
                    commands.entity(root).insert(local);
                }
            }
        }
    }
}

/// Registers the effect-application systems. Add to any binary that replays
/// published stacks (the game; the author preview rides the same systems).
pub struct EffectPlugin;

impl Plugin for EffectPlugin {
    fn build(&self, app: &mut App) {
        app.add_systems(Update, (apply_scale_tracks, apply_opacity_tracks));
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn key(tick: u32, value: f32, ease: EaseKind) -> EffectKey {
        EffectKey { tick, value, ease }
    }

    #[test]
    fn sampling_clamps_outside_and_eases_between() {
        let track = EffectTrack {
            kind: EffectKind::Scale,
            keys: vec![
                key(60, 1.0, EaseKind::Linear),
                key(120, 3.0, EaseKind::Linear),
            ],
        };
        assert_eq!(track.sample(0), Some(1.0));
        assert_eq!(track.sample(60), Some(1.0));
        assert_eq!(track.sample(90), Some(2.0)); // linear midpoint
        assert_eq!(track.sample(120), Some(3.0));
        assert_eq!(track.sample(7_000), Some(3.0));
        let empty = EffectTrack {
            kind: EffectKind::Opacity,
            keys: vec![],
        };
        assert_eq!(empty.sample(0), None);
    }

    #[test]
    fn hold_keys_step_at_the_segment_end() {
        let track = EffectTrack {
            kind: EffectKind::Opacity,
            keys: vec![key(0, 0.2, EaseKind::Hold), key(100, 0.9, EaseKind::Linear)],
        };
        assert_eq!(track.sample(0), Some(0.2));
        assert_eq!(track.sample(50), Some(0.2));
        assert_eq!(track.sample(99), Some(0.2));
        assert_eq!(track.sample(100), Some(0.9));
    }

    /// Every vocabulary entry maps to a bevy curve that samples sanely at the
    /// segment endpoints (start == a, end == b) — a new variant that forgets
    /// its mapping fails here, not at author time.
    #[test]
    fn every_ease_kind_maps_and_hits_its_endpoints() {
        const ALL: [EaseKind; 24] = [
            EaseKind::Linear,
            EaseKind::SineIn,
            EaseKind::SineOut,
            EaseKind::SineInOut,
            EaseKind::QuadIn,
            EaseKind::QuadOut,
            EaseKind::QuadInOut,
            EaseKind::CubicIn,
            EaseKind::CubicOut,
            EaseKind::CubicInOut,
            EaseKind::ExpoIn,
            EaseKind::ExpoOut,
            EaseKind::ExpoInOut,
            EaseKind::BackIn,
            EaseKind::BackOut,
            EaseKind::BackInOut,
            EaseKind::ElasticIn,
            EaseKind::ElasticOut,
            EaseKind::ElasticInOut,
            EaseKind::BounceIn,
            EaseKind::BounceOut,
            EaseKind::BounceInOut,
            EaseKind::SmoothStep,
            EaseKind::Hold,
        ];
        for ease in ALL {
            let track = EffectTrack {
                kind: EffectKind::Scale,
                keys: vec![key(0, 1.0, ease), key(100, 2.0, ease)],
            };
            assert_eq!(track.sample(0), Some(1.0), "{ease:?} start");
            assert_eq!(track.sample(100), Some(2.0), "{ease:?} end");
            let mid = track.sample(50).expect("mid sample");
            assert!(mid.is_finite(), "{ease:?} mid not finite: {mid}");
        }
    }

    #[test]
    fn tracks_round_trip_through_ron() {
        let track = EffectTrack {
            kind: EffectKind::Opacity,
            keys: vec![
                key(0, 1.0, EaseKind::CubicInOut),
                key(600, 0.0, EaseKind::Hold),
            ],
        };
        let text = ron::to_string(&track).unwrap();
        assert_eq!(ron::from_str::<EffectTrack>(&text).unwrap(), track);
    }
}
