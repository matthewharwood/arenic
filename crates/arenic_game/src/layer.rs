//! **Layers** — the authoring-side structure of an encounter
//! (`_docs/AUTHORING_UI.md` §1).
//!
//! A [`LayerStack`] is the ordered list of layers one `(arena, difficulty)`
//! owns: the boss staff, any number of minion layers, and any number of tile
//! choreography layers (instances — partitions, waves). Playback never sees
//! layers: at load/restart the stack **folds** into the one master
//! [`ArenaTimeline`] + one merged tile schedule, exactly the single staff the
//! sheet-music system replays.
//!
//! Invariants (locked in the design doc):
//! - **Top wins.** Fold applies layers bottom→top, so where two tile layers
//!   claim the same cell at the same tick, the layer HIGHER in the stack wins
//!   (AE stacking semantics — the existing `desired()` later-keyframe-wins
//!   rule lifted to layer granularity).
//! - **Mute folds to nothing; solo mutes the rest** (additive multi-solo).
//! - **Tick quantization.** Every event, keyframe, and spawn lives on a 60 Hz
//!   tick; there is no sub-tick anything.
//! - **Atomic publish.** The whole stack persists as one versioned
//!   `layers.vNNNN.ron`; rolling back is deleting the newest file, exactly
//!   like the legacy per-piece scores it replaces. Legacy `boss.vNNNN.ron` +
//!   `tiles.vNNNN.ron` pairs still READ (synthesized into a two-layer stack)
//!   so pre-layer encounters keep working; writers emit only the new format.

use bevy::prelude::*;
use serde::{Deserialize, Serialize};

use crate::arena::Arena;
use crate::effect::EffectTrack;
use crate::encounter::{Difficulty, ScoreIoError, validate_header};
use crate::tile_script::TileKeyframe;
use crate::timeline::{Action, ArenaTimeline, Recording, TimelineEvent, fold};

/// File prefix of a layered encounter score (`layers.vNNNN.ron`).
pub const LAYERS_PREFIX: &str = "layers";
/// The layered score format. Distinct from the legacy [`crate::encounter::SCORE_FORMAT`]
/// (which `boss.*`/`tiles.*` files keep): readers reject unknown formats loudly.
pub const LAYERS_FORMAT: u32 = 2;

/// A layer's stable identity within its stack (and its file). Never reused
/// within a stack; allocate with [`LayerStack::next_id`].
#[derive(Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash, Debug, Serialize, Deserialize)]
pub struct LayerId(pub u32);

/// What a minion looks like / how it behaves — grows per enemy design.
#[derive(Clone, Copy, PartialEq, Eq, Debug, Serialize, Deserialize)]
pub enum MinionArchetype {
    /// The v1 placeholder token (a small theme-tinted disc).
    Token,
}

/// A minion layer: where/when it exists, plus its recorded staff. Recording
/// ticks are ABSOLUTE cycle ticks (captured live against the arena clock,
/// same as the boss) — `spawn_tick..despawn_tick` is only the visibility
/// window the folder drives.
#[derive(Clone, PartialEq, Debug)]
pub struct MinionLayer {
    pub archetype: MinionArchetype,
    pub spawn_tick: u32,
    pub despawn_tick: u32,
    pub spawn_tile: IVec2,
    pub recording: Recording,
}

/// One layer's content.
#[derive(Clone, PartialEq, Debug)]
pub enum LayerKind {
    /// The boss staff (one per stack by convention, not by type).
    Boss(Recording),
    /// A spawned minion with its own staff.
    Minion(MinionLayer),
    /// One tile-choreography instance — N of these are legal (partitions,
    /// waves); stack order resolves their overlaps.
    Tiles(Vec<TileKeyframe>),
}

/// One authoring layer. `muted` and `effects` persist (a published stack
/// remembers them); `solo`/`locked` are session-side UI state and never
/// serialize.
#[derive(Clone, Debug)]
pub struct Layer {
    pub id: LayerId,
    pub name: String,
    pub kind: LayerKind,
    pub muted: bool,
    pub solo: bool,
    pub locked: bool,
    /// Non-destructive keyframed effects ([`crate::effect`]) applied to the
    /// layer's bound entity at render time.
    pub effects: Vec<EffectTrack>,
}

impl Layer {
    pub fn new(id: LayerId, name: impl Into<String>, kind: LayerKind) -> Self {
        Self {
            id,
            name: name.into(),
            kind,
            muted: false,
            solo: false,
            locked: false,
            effects: Vec::new(),
        }
    }
}

/// Marks an entity as the live binding of one layer (inserted at fold time —
/// the boss root today, minions next). The effect systems resolve a bound
/// entity's tracks through this.
#[derive(Component, Clone, Copy, Debug)]
#[component(immutable)]
pub struct LayerBinding(pub LayerId);

/// The ordered stack: index 0 = bottom, last = top (the panel renders top
/// first, AE-style). Fold applies bottom→top so the top layer wins conflicts.
#[derive(Clone, Debug, Default)]
pub struct LayerStack {
    pub layers: Vec<Layer>,
}

impl LayerStack {
    /// The next unused [`LayerId`] in this stack.
    pub fn next_id(&self) -> LayerId {
        LayerId(
            self.layers
                .iter()
                .map(|layer| layer.id.0)
                .max()
                .map_or(0, |max| max.strict_add(1)),
        )
    }

    pub fn layer(&self, id: LayerId) -> Option<&Layer> {
        self.layers.iter().find(|layer| layer.id == id)
    }

    pub fn layer_mut(&mut self, id: LayerId) -> Option<&mut Layer> {
        self.layers.iter_mut().find(|layer| layer.id == id)
    }

    /// The layers that participate in a fold, bottom→top: solo wins over
    /// mute (any solo present → only unmuted solos play), else every unmuted
    /// layer.
    pub fn effective(&self) -> impl Iterator<Item = &Layer> {
        let any_solo = self.layers.iter().any(|layer| layer.solo);
        self.layers
            .iter()
            .filter(move |layer| !layer.muted && (!any_solo || layer.solo))
    }

    /// The merged tile schedule, bottom→top — feeding the existing
    /// `tile_script::desired` evaluation, whose later-keyframe-wins rule makes
    /// the TOP layer win overlapping cells (the conflict invariant).
    pub fn merged_tiles(&self) -> Vec<TileKeyframe> {
        self.effective()
            .filter_map(|layer| match &layer.kind {
                LayerKind::Tiles(keyframes) => Some(keyframes.as_slice()),
                _ => None,
            })
            .flatten()
            .copied()
            .collect()
    }

    /// Every effective recording-bearing layer (boss + minions), bottom→top,
    /// with its staff — what [`fold_stack`] folds for the entities the caller
    /// bound.
    pub fn recordings(&self) -> impl Iterator<Item = (LayerId, &Recording)> {
        self.effective().filter_map(|layer| match &layer.kind {
            LayerKind::Boss(recording) => Some((layer.id, recording)),
            LayerKind::Minion(minion) => Some((layer.id, &minion.recording)),
            LayerKind::Tiles(_) => None,
        })
    }
}

/// An arena root's live stack: `stack` is what the world folds and the author
/// edits (the DRAFT in author builds); `published` is the baseline it diffs
/// against (== `stack` in plain-game builds, refreshed on every load). The
/// score sync inserts/refreshes this; the author's `W` publishes `stack` and
/// re-baselines.
#[derive(Component, Clone, Default)]
pub struct ArenaStack {
    pub stack: LayerStack,
    pub published: LayerStack,
    pub version: Option<u32>,
}

impl ArenaStack {
    /// A freshly-loaded stack: draft == published baseline.
    pub fn loaded(stack: LayerStack, version: u32) -> Self {
        Self {
            published: stack.clone(),
            stack,
            version: Some(version),
        }
    }

    /// `true` while `layer` differs from its published baseline (persisted
    /// fields only — `solo`/`locked` are session UI state).
    pub fn layer_dirty(&self, id: LayerId) -> bool {
        let persisted = |layer: &Layer| {
            (
                layer.name.clone(),
                layer.muted,
                layer.kind.clone(),
                layer.effects.clone(),
            )
        };
        match (self.stack.layer(id), self.published.layer(id)) {
            (Some(draft), Some(baseline)) => persisted(draft) != persisted(baseline),
            (None, None) => false,
            _ => true,
        }
    }

    /// `true` while ANY layer differs from the published baseline — unsaved
    /// author work the score sync must never destroy.
    pub fn dirty(&self) -> bool {
        let ids: std::collections::BTreeSet<u32> = self
            .stack
            .layers
            .iter()
            .chain(&self.published.layers)
            .map(|layer| layer.id.0)
            .collect();
        ids.into_iter().any(|id| self.layer_dirty(LayerId(id)))
    }

    /// Publishes the draft: baseline = draft, version = the new file version.
    pub fn mark_published(&mut self, version: u32) {
        self.published = self.stack.clone();
        self.version = Some(version);
    }
}

/// Folds every effective recording layer of `stack` into `timeline` using the
/// caller's `(LayerId → Entity)` bindings (the world owns entities; this fn
/// stays pure). Layers without a binding are skipped — the caller decides
/// what entity a layer drives. Deterministic: same stack + same bindings →
/// the same master timeline, byte for byte.
pub fn fold_stack(
    stack: &LayerStack,
    bindings: &[(LayerId, Entity)],
    timeline: &mut ArenaTimeline,
) {
    for (layer_id, recording) in stack.recordings() {
        if let Some(&(_, entity)) = bindings.iter().find(|(id, _)| *id == layer_id) {
            fold(timeline, entity, recording);
        }
    }
}

// --- File DTOs ---------------------------------------------------------------

/// A serialized action. Mirrors [`Action`] but stays file-stable: `Ability`
/// is a struct variant so future payloads (ARE-46's `aim`) extend it without
/// breaking arity. `aim` is carried through the file today and consumed by
/// playback once directional abilities land.
#[derive(Clone, Copy, PartialEq, Eq, Debug, Serialize, Deserialize)]
pub enum ActionDto {
    Move(i32, i32),
    Ability {
        slot: u8,
        #[serde(default)]
        aim: Option<(i32, i32)>,
    },
}

impl From<Action> for ActionDto {
    fn from(action: Action) -> Self {
        match action {
            Action::Move(delta) => ActionDto::Move(delta.x, delta.y),
            Action::Ability { slot, aim } => ActionDto::Ability {
                slot,
                aim: aim.map(|a| (a.x, a.y)),
            },
        }
    }
}

impl From<ActionDto> for Action {
    fn from(dto: ActionDto) -> Self {
        match dto {
            ActionDto::Move(x, y) => Action::Move(IVec2::new(x, y)),
            ActionDto::Ability { slot, aim } => Action::Ability {
                slot,
                aim: aim.map(|(x, y)| IVec2::new(x, y)),
            },
        }
    }
}

/// A serialized [`Recording`].
#[derive(Clone, PartialEq, Debug, Serialize, Deserialize)]
pub struct RecordingDto {
    pub start: (i32, i32),
    pub events: Vec<(u32, ActionDto)>,
}

impl RecordingDto {
    fn from_recording(recording: &Recording) -> Self {
        Self {
            start: (recording.start.x, recording.start.y),
            events: recording
                .events
                .iter()
                .map(|&TimelineEvent { tick, action }| (tick, action.into()))
                .collect(),
        }
    }

    fn recording(&self) -> Recording {
        Recording {
            start: IVec2::new(self.start.0, self.start.1),
            events: self
                .events
                .iter()
                .map(|&(tick, action)| TimelineEvent {
                    tick,
                    action: action.into(),
                })
                .collect(),
        }
    }
}

#[derive(Clone, PartialEq, Debug, Serialize, Deserialize)]
pub enum LayerKindDto {
    Boss(RecordingDto),
    Minion {
        archetype: MinionArchetype,
        spawn_tick: u32,
        despawn_tick: u32,
        spawn_tile: (i32, i32),
        recording: RecordingDto,
    },
    Tiles(Vec<TileKeyframe>),
}

#[derive(Clone, PartialEq, Debug, Serialize, Deserialize)]
pub struct LayerDto {
    pub id: u32,
    pub name: String,
    #[serde(default)]
    pub muted: bool,
    #[serde(default)]
    pub effects: Vec<EffectTrack>,
    pub kind: LayerKindDto,
}

/// One whole stack on disk — the atomic publish unit.
#[derive(Clone, PartialEq, Debug, Serialize, Deserialize)]
pub struct LayerScoreFile {
    pub format: u32,
    pub arena: u8,
    pub difficulty: Difficulty,
    pub layers: Vec<LayerDto>,
}

impl LayerScoreFile {
    pub fn from_stack(arena: Arena, difficulty: Difficulty, stack: &LayerStack) -> Self {
        Self {
            format: LAYERS_FORMAT,
            arena: arena.index() as u8,
            difficulty,
            layers: stack
                .layers
                .iter()
                .map(|layer| LayerDto {
                    id: layer.id.0,
                    name: layer.name.clone(),
                    muted: layer.muted,
                    effects: layer.effects.clone(),
                    kind: match &layer.kind {
                        LayerKind::Boss(recording) => {
                            LayerKindDto::Boss(RecordingDto::from_recording(recording))
                        }
                        LayerKind::Minion(minion) => LayerKindDto::Minion {
                            archetype: minion.archetype,
                            spawn_tick: minion.spawn_tick,
                            despawn_tick: minion.despawn_tick,
                            spawn_tile: (minion.spawn_tile.x, minion.spawn_tile.y),
                            recording: RecordingDto::from_recording(&minion.recording),
                        },
                        LayerKind::Tiles(keyframes) => LayerKindDto::Tiles(keyframes.clone()),
                    },
                })
                .collect(),
        }
    }

    pub fn stack(&self) -> LayerStack {
        LayerStack {
            layers: self
                .layers
                .iter()
                .map(|dto| Layer {
                    id: LayerId(dto.id),
                    name: dto.name.clone(),
                    muted: dto.muted,
                    solo: false,
                    locked: false,
                    effects: dto.effects.clone(),
                    kind: match &dto.kind {
                        LayerKindDto::Boss(recording) => LayerKind::Boss(recording.recording()),
                        LayerKindDto::Minion {
                            archetype,
                            spawn_tick,
                            despawn_tick,
                            spawn_tile,
                            recording,
                        } => LayerKind::Minion(MinionLayer {
                            archetype: *archetype,
                            spawn_tick: *spawn_tick,
                            despawn_tick: *despawn_tick,
                            spawn_tile: IVec2::new(spawn_tile.0, spawn_tile.1),
                            recording: recording.recording(),
                        }),
                        LayerKindDto::Tiles(keyframes) => LayerKind::Tiles(keyframes.clone()),
                    },
                })
                .collect(),
        }
    }

    /// Rejects a header the reader can't trust — same contract as the legacy
    /// score files.
    pub fn validate(&self, arena: Arena, difficulty: Difficulty) -> Result<(), ScoreIoError> {
        validate_header(
            self.format,
            LAYERS_FORMAT,
            self.arena,
            self.difficulty,
            arena,
            difficulty,
        )
    }
}

// --- Reading a stack from disk (native only) ----------------------------------

/// The newest published stack in `dir`: the layered file when one exists,
/// otherwise a stack SYNTHESIZED from the legacy `boss.vNNNN.ron` +
/// `tiles.vNNNN.ron` pair (so pre-layer encounters keep replaying). Returns
/// `(stack_version, stack)`; for legacy synthesis the version is the max of
/// the two legacy versions. `None` when nothing is published at all.
#[cfg(not(target_arch = "wasm32"))]
pub fn read_stack(
    dir: &std::path::Path,
    arena: Arena,
    difficulty: Difficulty,
) -> Result<Option<(u32, LayerStack)>, ScoreIoError> {
    use crate::encounter::{BOSS_PREFIX, BossScoreFile, latest_version, read_ron};
    use crate::tile_script::{TILES_PREFIX, TileScriptFile};

    if let Some((version, path)) = latest_version(dir, LAYERS_PREFIX) {
        let file: LayerScoreFile = read_ron(&path)?;
        file.validate(arena, difficulty)?;
        return Ok(Some((version, file.stack())));
    }

    // Legacy fallback: synthesize Boss + Tiles layers from the old pair.
    let mut stack = LayerStack::default();
    let mut version = 0u32;
    if let Some((boss_version, path)) = latest_version(dir, BOSS_PREFIX) {
        let file: BossScoreFile = read_ron(&path)?;
        file.validate(arena, difficulty)?;
        stack.layers.push(Layer::new(
            stack.next_id(),
            "Boss",
            LayerKind::Boss(file.recording()),
        ));
        version = version.max(boss_version);
    }
    if let Some((tiles_version, path)) = latest_version(dir, TILES_PREFIX) {
        let file: TileScriptFile = read_ron(&path)?;
        file.validate(arena, difficulty)?;
        stack.layers.push(Layer::new(
            stack.next_id(),
            "Tiles",
            LayerKind::Tiles(file.keyframes),
        ));
        version = version.max(tiles_version);
    }
    Ok((!stack.layers.is_empty()).then_some((version, stack)))
}

/// Publishes `stack` as the next `layers.vNNNN.ron` in `dir`, returning the
/// `(version, path)` it landed at.
#[cfg(not(target_arch = "wasm32"))]
pub fn write_stack(
    dir: &std::path::Path,
    arena: Arena,
    difficulty: Difficulty,
    stack: &LayerStack,
) -> Result<(u32, std::path::PathBuf), ScoreIoError> {
    let file = LayerScoreFile::from_stack(arena, difficulty, stack);
    crate::encounter::write_versioned_ron(dir, LAYERS_PREFIX, &file)
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::tile::TileKind;
    use crate::tile_script::{TileSelector, desired};

    fn rec(start: (i32, i32), ticks: &[u32]) -> Recording {
        Recording {
            start: IVec2::new(start.0, start.1),
            events: ticks
                .iter()
                .map(|&tick| TimelineEvent {
                    tick,
                    action: Action::Move(IVec2::X),
                })
                .collect(),
        }
    }

    fn cell_frame(from: u32, to: u32, col: u8, row: u8, kind: TileKind) -> TileKeyframe {
        TileKeyframe {
            from,
            to,
            selector: TileSelector::Cell { col, row },
            kind,
        }
    }

    fn two_tile_stack() -> LayerStack {
        // Bottom layer lavas (2,2); the TOP layer holds the same cell Normal.
        let mut stack = LayerStack::default();
        stack.layers.push(Layer::new(
            LayerId(0),
            "bottom",
            LayerKind::Tiles(vec![cell_frame(0, 100, 2, 2, TileKind::Lava)]),
        ));
        stack.layers.push(Layer::new(
            LayerId(1),
            "top",
            LayerKind::Tiles(vec![cell_frame(0, 100, 2, 2, TileKind::Normal)]),
        ));
        stack
    }

    #[test]
    fn top_layer_wins_overlapping_cells() {
        let stack = two_tile_stack();
        let merged = stack.merged_tiles();
        assert_eq!(desired(&merged, 50)[&(2, 2)], TileKind::Normal);
        // Reversing the stack flips the winner — order IS the rule.
        let mut reversed = stack.clone();
        reversed.layers.reverse();
        assert_eq!(
            desired(&reversed.merged_tiles(), 50)[&(2, 2)],
            TileKind::Lava
        );
    }

    #[test]
    fn mute_and_solo_filter_the_fold() {
        let mut stack = two_tile_stack();
        stack.layers[1].muted = true;
        assert_eq!(desired(&stack.merged_tiles(), 50)[&(2, 2)], TileKind::Lava);
        // Solo on the bottom layer mutes the (unmuted) top one.
        stack.layers[1].muted = false;
        stack.layers[0].solo = true;
        assert_eq!(desired(&stack.merged_tiles(), 50)[&(2, 2)], TileKind::Lava);
    }

    #[test]
    fn fold_stack_is_deterministic_and_binding_scoped() {
        let mut world = World::new();
        let (boss, minion) = (world.spawn_empty().id(), world.spawn_empty().id());
        let mut stack = LayerStack::default();
        stack.layers.push(Layer::new(
            LayerId(0),
            "Boss",
            LayerKind::Boss(rec((3, 3), &[5, 10])),
        ));
        stack.layers.push(Layer::new(
            LayerId(1),
            "Minion A",
            LayerKind::Minion(MinionLayer {
                archetype: MinionArchetype::Token,
                spawn_tick: 60,
                despawn_tick: 600,
                spawn_tile: IVec2::new(1, 1),
                recording: rec((1, 1), &[60, 70]),
            }),
        ));
        let bindings = [(LayerId(0), boss), (LayerId(1), minion)];
        let (mut a, mut b) = (ArenaTimeline::default(), ArenaTimeline::default());
        fold_stack(&stack, &bindings, &mut a);
        fold_stack(&stack, &bindings, &mut b);
        let ticks: Vec<(u32, Entity)> = a.events.iter().map(|e| (e.tick, e.ghost)).collect();
        assert_eq!(
            ticks,
            b.events
                .iter()
                .map(|e| (e.tick, e.ghost))
                .collect::<Vec<_>>()
        );
        assert_eq!(
            ticks,
            vec![(5, boss), (10, boss), (60, minion), (70, minion)]
        );
        // An unbound layer folds nothing rather than panicking.
        let mut partial = ArenaTimeline::default();
        fold_stack(&stack, &[(LayerId(0), boss)], &mut partial);
        assert!(partial.events.iter().all(|e| e.ghost == boss));
    }

    #[test]
    fn layered_file_round_trips_and_validates() {
        let mut stack = two_tile_stack();
        stack.layers.push(Layer::new(
            LayerId(2),
            "Boss",
            LayerKind::Boss(rec((33, 15), &[0, 90])),
        ));
        stack.layers[0].muted = true;
        let file = LayerScoreFile::from_stack(Arena::Hunter, Difficulty::Normal, &stack);
        assert!(file.validate(Arena::Hunter, Difficulty::Normal).is_ok());
        assert!(file.validate(Arena::Bard, Difficulty::Normal).is_err());
        let text = ron::ser::to_string_pretty(&file, ron::ser::PrettyConfig::default()).unwrap();
        let back: LayerScoreFile = ron::from_str(&text).unwrap();
        assert_eq!(back, file);
        let restored = back.stack();
        assert_eq!(restored.layers.len(), 3);
        assert!(restored.layers[0].muted);
        assert_eq!(restored.layers[2].name, "Boss");
    }

    #[test]
    fn aim_payload_round_trips_action_file_action() {
        // Sim -> file -> sim preserves the aim exactly…
        let aimed = Action::Ability {
            slot: 2,
            aim: Some(IVec2::new(1, -1)),
        };
        let dto: ActionDto = aimed.into();
        let text = ron::to_string(&dto).unwrap();
        let back: Action = ron::from_str::<ActionDto>(&text).unwrap().into();
        assert_eq!(back, aimed);
        // …and an aim-less row from an older stack still parses (serde default).
        let legacy: ActionDto = ron::from_str("Ability(slot: 3)").unwrap();
        assert_eq!(Action::from(legacy), Action::Ability { slot: 3, aim: None });
    }

    #[cfg(not(target_arch = "wasm32"))]
    #[test]
    fn legacy_pair_synthesizes_a_stack_and_layered_file_wins() {
        use crate::encounter::{BossScoreFile, write_versioned_ron};
        use crate::tile_script::TileScriptFile;

        let dir = std::env::temp_dir().join(format!(
            "arenic-layer-test-{}-{}",
            std::process::id(),
            std::time::UNIX_EPOCH.elapsed().unwrap().as_nanos()
        ));
        std::fs::create_dir_all(&dir).unwrap();
        let (arena, difficulty) = (Arena::Hunter, Difficulty::Normal);

        // Nothing published yet.
        assert!(read_stack(&dir, arena, difficulty).unwrap().is_none());

        // Legacy pair → synthesized two-layer stack at max(versions).
        let boss = BossScoreFile::from_recording(arena, difficulty, &rec((33, 15), &[5]));
        write_versioned_ron(&dir, crate::encounter::BOSS_PREFIX, &boss).unwrap();
        let tiles = TileScriptFile {
            format: crate::encounter::SCORE_FORMAT,
            arena: arena.index() as u8,
            difficulty,
            keyframes: vec![cell_frame(0, 60, 1, 1, TileKind::Lava)],
        };
        write_versioned_ron(&dir, crate::tile_script::TILES_PREFIX, &tiles).unwrap();
        write_versioned_ron(&dir, crate::tile_script::TILES_PREFIX, &tiles).unwrap();
        let (version, stack) = read_stack(&dir, arena, difficulty).unwrap().unwrap();
        assert_eq!(version, 2); // max(boss v1, tiles v2)
        assert_eq!(stack.layers.len(), 2);
        assert!(matches!(stack.layers[0].kind, LayerKind::Boss(_)));
        assert!(matches!(stack.layers[1].kind, LayerKind::Tiles(_)));

        // A layered file takes precedence over the legacy pair.
        let (published, _) = write_stack(&dir, arena, difficulty, &stack).unwrap();
        assert_eq!(published, 1);
        let (version, layered) = read_stack(&dir, arena, difficulty).unwrap().unwrap();
        assert_eq!(version, 1);
        assert_eq!(layered.layers.len(), 2);

        std::fs::remove_dir_all(&dir).unwrap();
    }
}
