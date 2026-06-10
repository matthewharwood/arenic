//! The **boss encounter framework** — difficulties, phases, ability loadouts,
//! and the versioned score files that carry an authored boss timeline from the
//! author tool to the game.
//!
//! Every boss fight is one 2-minute arena cycle split into [`PHASES`] equal
//! phases; each phase hands the boss [`ABILITY_SLOTS`] ability slots; and each
//! [`Difficulty`] (Normal → Heroic → Mythic, RULEBOOK → The Goal) owns its
//! **own timeline** — its own loadout row *and* its own authored score files.
//! A boss ability is the same data structure as a hero ability
//! ([`crate::ability::AbilityId`]), so the two can never drift.
//!
//! Authored timelines are plain RON files under
//! `assets/encounters/<arena-slug>/<difficulty>/`, versioned as
//! `boss.v0001.ron`, `boss.v0002.ron`, … (tile choreography:
//! `tiles.vNNNN.ron`, see [`crate::tile_script`]). The author tool always
//! writes `latest + 1`; readers always load the highest version; rolling back
//! is deleting the newest file. The files are committed to git, so history is
//! the second level of versioning.

#[cfg(not(target_arch = "wasm32"))]
use std::path::PathBuf;

use bevy::prelude::*;
#[cfg(not(target_arch = "wasm32"))]
use serde::de::DeserializeOwned;
use serde::{Deserialize, Serialize};

use crate::ability::AbilityId;
use crate::arena::Arena;
use crate::timeline::{Action, CYCLE_TICKS, Recording, TimelineEvent};

/// One of the three encounter difficulties. Each owns its own boss/tile
/// timelines and its own [`loadout`] row — a difficulty can change ability
/// statistics or the whole encounter, per the design.
#[derive(Clone, Copy, PartialEq, Eq, Debug, Default, Serialize, Deserialize)]
pub enum Difficulty {
    #[default]
    Normal,
    Heroic,
    Mythic,
}

impl Difficulty {
    /// All difficulties, in ascending order.
    pub const ALL: [Difficulty; 3] = [Difficulty::Normal, Difficulty::Heroic, Difficulty::Mythic];

    /// The difficulty's directory slug under an arena's encounter folder.
    pub fn slug(self) -> &'static str {
        match self {
            Difficulty::Normal => "normal",
            Difficulty::Heroic => "heroic",
            Difficulty::Mythic => "mythic",
        }
    }

    /// The next difficulty, cyclic — the author tool's `D` key.
    pub fn next(self) -> Difficulty {
        match self {
            Difficulty::Normal => Difficulty::Heroic,
            Difficulty::Heroic => Difficulty::Mythic,
            Difficulty::Mythic => Difficulty::Normal,
        }
    }
}

/// The difficulty the game is currently playing (and the author tool is
/// currently authoring). Score loading and boss ability resolution both read
/// this; swapping it re-points every arena at that difficulty's timelines.
#[derive(Resource, Clone, Copy, PartialEq, Eq, Debug, Default)]
pub struct ActiveDifficulty(pub Difficulty);

/// Phases per 2-minute cycle.
pub const PHASES: usize = 3;
/// Ability slots a boss holds per phase (mirrors the hero's `1..=4` keys).
pub const ABILITY_SLOTS: usize = 4;
/// Ticks per phase — equal thirds of the cycle (const-evaluated).
pub const PHASE_TICKS: u32 = CYCLE_TICKS / PHASES as u32;

/// The phase (`0..PHASES`) a cycle tick falls in — the ONE place phase
/// boundaries live.
pub fn phase_at(tick: u32) -> usize {
    ((tick / PHASE_TICKS) as usize).min(PHASES.strict_sub(1))
}

/// One phase's four ability slots.
pub type PhaseLoadout = [AbilityId; ABILITY_SLOTS];

/// The boss's ability loadout for one `(arena, difficulty)` — its source of
/// truth, mirroring the [`Arena::spec`] table pattern. This is the tree that
/// grows per boss as real abilities land: today every slot of every phase of
/// every boss on every difficulty is Holy Nova (the only ability that exists),
/// locked in so timelines can be authored against the real shape.
pub fn loadout(arena: Arena, difficulty: Difficulty) -> [PhaseLoadout; PHASES] {
    // Placeholder: one uniform tree. Branch on `arena`/`difficulty` per boss
    // as abilities are designed — the signature is the contract.
    let _ = (arena, difficulty);
    [[AbilityId::HolyNova; ABILITY_SLOTS]; PHASES]
}

/// The ability an `Action::Ability(slot)` playback event resolves to at `tick`.
/// Boss ghosts read the slot from their phase loadout; heroes have only slot 1
/// (Holy Nova) today, with slots 2-4 recorded but inert.
pub fn resolve_ability(
    is_boss: bool,
    arena: Arena,
    difficulty: Difficulty,
    slot: u8,
    tick: u32,
) -> Option<AbilityId> {
    if is_boss {
        let index = slot.checked_sub(1)? as usize;
        loadout(arena, difficulty)[phase_at(tick)]
            .get(index)
            .copied()
    } else {
        (slot == 1).then_some(AbilityId::HolyNova)
    }
}

// --- Score files -----------------------------------------------------------

/// Bumped when the file schema changes; readers reject unknown formats loudly
/// instead of misreading an old take.
pub const SCORE_FORMAT: u32 = 1;

/// File prefix of a boss movement/ability score (`boss.vNNNN.ron`).
pub const BOSS_PREFIX: &str = "boss";

/// One serialized [`Action`] — plain tuples so the file format never depends
/// on bevy's serde features.
#[derive(Clone, Copy, PartialEq, Eq, Debug, Serialize, Deserialize)]
pub enum EventDto {
    Move(i32, i32),
    Ability(u8),
}

impl From<Action> for EventDto {
    fn from(action: Action) -> Self {
        match action {
            Action::Move(delta) => EventDto::Move(delta.x, delta.y),
            Action::Ability(slot) => EventDto::Ability(slot),
        }
    }
}

impl From<EventDto> for Action {
    fn from(dto: EventDto) -> Self {
        match dto {
            EventDto::Move(x, y) => Action::Move(IVec2::new(x, y)),
            EventDto::Ability(slot) => Action::Ability(slot),
        }
    }
}

/// One authored boss timeline on disk: the boss's tick-0 tile plus its
/// tick-stamped intent events — exactly a hero's [`Recording`], made portable.
/// `arena`/`difficulty` restate the directory the file lives in, so a stray
/// file is self-describing.
#[derive(Clone, PartialEq, Debug, Serialize, Deserialize)]
pub struct BossScoreFile {
    pub format: u32,
    pub arena: u8,
    pub difficulty: Difficulty,
    pub start: (i32, i32),
    pub events: Vec<(u32, EventDto)>,
}

impl BossScoreFile {
    /// Rejects a header the reader can't trust: an unknown [`SCORE_FORMAT`]
    /// (schema drift — refusing loudly beats silently misreading an old take)
    /// or a file that drifted into another `(arena, difficulty)` directory.
    pub fn validate(&self, arena: Arena, difficulty: Difficulty) -> Result<(), ScoreIoError> {
        validate_header(
            self.format,
            SCORE_FORMAT,
            self.arena,
            self.difficulty,
            arena,
            difficulty,
        )
    }

    /// Wraps a committed [`Recording`] for writing.
    pub fn from_recording(arena: Arena, difficulty: Difficulty, recording: &Recording) -> Self {
        Self {
            format: SCORE_FORMAT,
            arena: arena.index() as u8,
            difficulty,
            start: (recording.start.x, recording.start.y),
            events: recording
                .events
                .iter()
                .map(|&TimelineEvent { tick, action }| (tick, action.into()))
                .collect(),
        }
    }

    /// The [`Recording`] this file carries — ready to fold into an arena's
    /// master timeline.
    pub fn recording(&self) -> Recording {
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

/// The shared header check behind [`BossScoreFile::validate`],
/// [`crate::tile_script::TileScriptFile::validate`], and
/// [`crate::layer::LayerScoreFile::validate`]: the file must speak the
/// reader's `expected` format and belong to the `(arena, difficulty)`
/// directory it was found in.
pub(crate) fn validate_header(
    format: u32,
    expected: u32,
    file_arena: u8,
    file_difficulty: Difficulty,
    arena: Arena,
    difficulty: Difficulty,
) -> Result<(), ScoreIoError> {
    if format != expected {
        return Err(format!("score format {format} (this reader speaks {expected})").into());
    }
    if file_arena != arena.index() as u8 || file_difficulty != difficulty {
        return Err(format!(
            "header says arena {file_arena} / {file_difficulty:?}, \
             directory says {} / {difficulty:?}",
            arena.index(),
        )
        .into());
    }
    Ok(())
}

// --- Versioned-file naming (pure, unit-tested) ------------------------------

/// The file name of version `n` of a score: `<prefix>.v0007.ron`.
pub fn version_file_name(prefix: &str, n: u32) -> String {
    format!("{prefix}.v{n:04}.ron")
}

/// Parses a score file name back to its version: `boss.v0007.ron` → `7` for
/// prefix `boss`. Strict about shape, lenient about digit count (hand-renamed
/// `boss.v12.ron` still reads).
pub fn parse_version(name: &str, prefix: &str) -> Option<u32> {
    let digits = name
        .strip_prefix(prefix)?
        .strip_prefix(".v")?
        .strip_suffix(".ron")?;
    (!digits.is_empty() && digits.bytes().all(|b| b.is_ascii_digit()))
        .then(|| digits.parse().ok())?
}

/// The highest version among `names` for `prefix` — the pure core of
/// [`latest_version`].
pub fn max_version<'a>(names: impl Iterator<Item = &'a str>, prefix: &str) -> Option<u32> {
    names.filter_map(|n| parse_version(n, prefix)).max()
}

// --- Filesystem IO (native only) ---------------------------------------------
//
// The web build ships none of this: score files are read with `std::fs` from
// the workspace's `assets/` (a baked manifest for wasm is future work).

/// Any error a score read/write can produce (io or RON) — converts into
/// `BevyError` via `?` in fallible systems.
pub type ScoreIoError = Box<dyn core::error::Error + Send + Sync>;

/// Root of every versioned encounter file: `<asset root>/assets/encounters`.
/// `.cargo/config.toml` pins `BEVY_ASSET_ROOT` to the workspace root, so the
/// scores sit beside the other shared assets and are committed to git. The
/// fallbacks mirror Bevy's `FileAssetReader` chain (`CARGO_MANIFEST_DIR`, then
/// the executable's directory) so score files and the assets they sit beside
/// always resolve to the SAME root — also in a shipped build run outside cargo.
#[cfg(not(target_arch = "wasm32"))]
pub fn encounters_root() -> PathBuf {
    let root = std::env::var_os("BEVY_ASSET_ROOT")
        .or_else(|| std::env::var_os("CARGO_MANIFEST_DIR"))
        .map(PathBuf::from)
        .unwrap_or_else(|| {
            std::env::current_exe()
                .ok()
                .and_then(|exe| Some(exe.parent()?.to_path_buf()))
                .unwrap_or_default()
        });
    root.join("assets").join("encounters")
}

/// The directory holding one `(arena, difficulty)`'s score files.
#[cfg(not(target_arch = "wasm32"))]
pub fn encounter_dir(arena: Arena, difficulty: Difficulty) -> PathBuf {
    encounters_root().join(arena.slug()).join(difficulty.slug())
}

/// The newest `(version, path)` of `prefix` in `dir`, or `None` when the
/// directory is missing or holds no parseable version. The path is the entry's
/// ACTUAL name, not a reconstruction — a hand-renamed `boss.v12.ron` (lenient
/// parse, see [`parse_version`]) must resolve to the file that exists.
#[cfg(not(target_arch = "wasm32"))]
pub fn latest_version(dir: &std::path::Path, prefix: &str) -> Option<(u32, PathBuf)> {
    let names: Vec<String> = std::fs::read_dir(dir)
        .ok()?
        .filter_map(|entry| Some(entry.ok()?.file_name().to_str()?.to_owned()))
        .collect();
    let v = max_version(names.iter().map(String::as_str), prefix)?;
    // Two spellings can tie (`boss.v12.ron` / `boss.v0012.ron`); `min` keeps
    // the pick deterministic.
    let name = names
        .iter()
        .filter(|name| parse_version(name, prefix) == Some(v))
        .min()?;
    Some((v, dir.join(name)))
}

/// Reads and parses one RON score file.
#[cfg(not(target_arch = "wasm32"))]
pub fn read_ron<T: DeserializeOwned>(path: &std::path::Path) -> Result<T, ScoreIoError> {
    Ok(ron::from_str(&std::fs::read_to_string(path)?)?)
}

/// Writes `value` as the **next** version of `prefix` in `dir` (creating the
/// directory if needed) and returns the `(version, path)` it landed at.
#[cfg(not(target_arch = "wasm32"))]
pub fn write_versioned_ron<T: Serialize>(
    dir: &std::path::Path,
    prefix: &str,
    value: &T,
) -> Result<(u32, PathBuf), ScoreIoError> {
    std::fs::create_dir_all(dir)?;
    let next = latest_version(dir, prefix).map_or(1, |(v, _)| v.strict_add(1));
    let path = dir.join(version_file_name(prefix, next));
    let text = ron::ser::to_string_pretty(value, ron::ser::PrettyConfig::default())?;
    std::fs::write(&path, text)?;
    Ok((next, path))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn phases_are_equal_thirds_of_the_cycle() {
        assert_eq!(phase_at(0), 0);
        assert_eq!(phase_at(PHASE_TICKS - 1), 0);
        assert_eq!(phase_at(PHASE_TICKS), 1);
        assert_eq!(phase_at(2 * PHASE_TICKS - 1), 1);
        assert_eq!(phase_at(2 * PHASE_TICKS), 2);
        assert_eq!(phase_at(CYCLE_TICKS - 1), 2);
        // Out-of-range ticks clamp to the last phase rather than indexing past it.
        assert_eq!(phase_at(CYCLE_TICKS), 2);
    }

    #[test]
    fn every_slot_of_every_phase_is_holy_nova_for_now() {
        for arena in Arena::ALL {
            for difficulty in Difficulty::ALL {
                for phase in loadout(arena, difficulty) {
                    assert_eq!(phase, [AbilityId::HolyNova; ABILITY_SLOTS]);
                }
            }
        }
    }

    #[test]
    fn ability_resolution_distinguishes_boss_and_hero() {
        let (arena, diff) = (Arena::Hunter, Difficulty::Mythic);
        // Heroes: only slot 1 casts today.
        assert_eq!(
            resolve_ability(false, arena, diff, 1, 0),
            Some(AbilityId::HolyNova)
        );
        assert_eq!(resolve_ability(false, arena, diff, 3, 0), None);
        // Bosses: all four slots resolve through the phase loadout…
        for slot in 1..=ABILITY_SLOTS as u8 {
            assert_eq!(
                resolve_ability(true, arena, diff, slot, CYCLE_TICKS - 1),
                Some(AbilityId::HolyNova)
            );
        }
        // …and out-of-range slots are inert, not a panic.
        assert_eq!(resolve_ability(true, arena, diff, 0, 0), None);
        assert_eq!(resolve_ability(true, arena, diff, 5, 0), None);
    }

    #[test]
    fn version_names_round_trip_and_parse_strictly() {
        assert_eq!(version_file_name("boss", 7), "boss.v0007.ron");
        assert_eq!(parse_version("boss.v0007.ron", "boss"), Some(7));
        assert_eq!(parse_version("boss.v12.ron", "boss"), Some(12));
        // Wrong prefix, missing digits, or stray shapes never parse.
        assert_eq!(parse_version("tiles.v0001.ron", "boss"), None);
        assert_eq!(parse_version("boss.ron", "boss"), None);
        assert_eq!(parse_version("boss.v.ron", "boss"), None);
        assert_eq!(parse_version("boss.v00x7.ron", "boss"), None);
    }

    #[test]
    fn max_version_picks_the_newest_and_skips_strangers() {
        let names = [
            "boss.v0001.ron",
            "boss.v0010.ron",
            "tiles.v9999.ron",
            "notes.txt",
        ];
        assert_eq!(max_version(names.into_iter(), "boss"), Some(10));
        assert_eq!(max_version(names.into_iter(), "tiles"), Some(9999));
        assert_eq!(max_version([].into_iter(), "boss"), None);
    }

    /// The author→game loop against the real filesystem: write v1 and v2, read
    /// the latest back, roll back by deleting v2 — the exact iterate/undo
    /// workflow author mode promises.
    #[test]
    fn versioned_files_round_trip_on_disk() {
        let dir = std::env::temp_dir().join(format!(
            "arenic-encounter-test-{}-{}",
            std::process::id(),
            std::time::UNIX_EPOCH.elapsed().unwrap().as_nanos()
        ));
        let recording = Recording {
            start: IVec2::new(3, 4),
            events: vec![TimelineEvent {
                tick: 5,
                action: Action::Ability(2),
            }]
            .into(),
        };
        let take = |tick| BossScoreFile {
            events: vec![(tick, EventDto::Ability(1))],
            ..BossScoreFile::from_recording(Arena::Bard, Difficulty::Normal, &recording)
        };

        assert_eq!(latest_version(&dir, BOSS_PREFIX), None);
        let (v1, _) = write_versioned_ron(&dir, BOSS_PREFIX, &take(1)).unwrap();
        let (v2, p2) = write_versioned_ron(&dir, BOSS_PREFIX, &take(2)).unwrap();
        assert_eq!((v1, v2), (1, 2));

        let (latest, path) = latest_version(&dir, BOSS_PREFIX).unwrap();
        assert_eq!((latest, &path), (2, &p2));
        assert_eq!(read_ron::<BossScoreFile>(&path).unwrap(), take(2));

        // Roll back: delete the newest file and the previous take is live again.
        std::fs::remove_file(&p2).unwrap();
        let (latest, path) = latest_version(&dir, BOSS_PREFIX).unwrap();
        assert_eq!(latest, 1);
        assert_eq!(read_ron::<BossScoreFile>(&path).unwrap(), take(1));

        std::fs::remove_dir_all(&dir).unwrap();
    }

    #[test]
    fn readers_reject_foreign_headers() {
        let recording = Recording {
            start: IVec2::new(1, 1),
            events: vec![].into(),
        };
        let file = BossScoreFile::from_recording(Arena::Bard, Difficulty::Normal, &recording);
        assert!(file.validate(Arena::Bard, Difficulty::Normal).is_ok());
        // A file that drifted into another arena's or difficulty's directory.
        assert!(file.validate(Arena::Hunter, Difficulty::Normal).is_err());
        assert!(file.validate(Arena::Bard, Difficulty::Mythic).is_err());
        // A future-format file is refused loudly, never misread.
        let newer = BossScoreFile {
            format: SCORE_FORMAT + 1,
            ..file
        };
        assert!(newer.validate(Arena::Bard, Difficulty::Normal).is_err());
    }

    /// A hand-renamed version (the lenient `boss.v12.ron` spelling the parser
    /// accepts) must resolve to the file that actually exists on disk.
    #[test]
    fn hand_renamed_versions_resolve_to_their_actual_file() {
        let dir = std::env::temp_dir().join(format!(
            "arenic-encounter-rename-test-{}-{}",
            std::process::id(),
            std::time::UNIX_EPOCH.elapsed().unwrap().as_nanos()
        ));
        std::fs::create_dir_all(&dir).unwrap();
        std::fs::write(dir.join("boss.v12.ron"), "stub").unwrap();
        let (version, path) = latest_version(&dir, BOSS_PREFIX).unwrap();
        assert_eq!(version, 12);
        assert!(
            path.exists(),
            "must return the real entry, not boss.v0012.ron"
        );
        std::fs::remove_dir_all(&dir).unwrap();
    }

    #[test]
    fn boss_score_round_trips_through_ron() {
        let recording = Recording {
            start: IVec2::new(33, 15),
            events: vec![
                TimelineEvent {
                    tick: 0,
                    action: Action::Move(IVec2::X),
                },
                TimelineEvent {
                    tick: 90,
                    action: Action::Ability(3),
                },
            ]
            .into(),
        };
        let file = BossScoreFile::from_recording(Arena::Hunter, Difficulty::Heroic, &recording);
        let text = ron::ser::to_string_pretty(&file, ron::ser::PrettyConfig::default()).unwrap();
        let back: BossScoreFile = ron::from_str(&text).unwrap();
        assert_eq!(back, file);
        let replay = back.recording();
        assert_eq!(replay.start, recording.start);
        assert_eq!(&replay.events[..], &recording.events[..]);
    }
}
