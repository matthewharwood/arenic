//! **Tile choreography** — keyframed board-state scripts, the tile half of the
//! sheet-music system.
//!
//! A [`TileKeyframe`] holds a tick range, a [`TileSelector`] (one cell, a
//! row/column, a rect, or a moving sine wave), and the [`TileKind`] those cells
//! wear while the range is active. The whole script is a deterministic
//! function of the arena clock: [`desired`] maps `(keyframes, tick)` to the
//! exact set of non-`Normal` cells, so playback, scrubbing, and replay can
//! never disagree. Scripts are authored in the author tool (or by hand), saved
//! as versioned `tiles.vNNNN.ron` files next to the boss scores
//! ([`crate::encounter`]), and applied to the board each tick by the game via
//! [`crate::tile::TileBoard`].

use bevy::platform::collections::HashMap;
use bevy::prelude::*;
use serde::{Deserialize, Serialize};

use crate::arena::Arena;
use crate::encounter::{Difficulty, ScoreIoError, validate_header};
use crate::grid::{GRID_H, GRID_W, MAX_COL, MAX_ROW};
use crate::tile::TileKind;
use crate::timeline::TICKS_PER_SECOND;

/// File prefix of a tile choreography score (`tiles.vNNNN.ron`).
pub const TILES_PREFIX: &str = "tiles";

/// Which cells a keyframe touches. `Cell`/`Row`/`Col`/`Rect` are static
/// selections; `SineWave` is a query — a band of tiles riding a sine across
/// the arena, animated over the keyframe's lifetime.
#[derive(Clone, Copy, PartialEq, Debug, Serialize, Deserialize)]
pub enum TileSelector {
    /// One cell.
    Cell { col: u8, row: u8 },
    /// Every cell of one row.
    Row { row: u8 },
    /// Every cell of one column.
    Col { col: u8 },
    /// Every cell in the inclusive rect spanned by the two corners (any order).
    Rect { c0: u8, r0: u8, c1: u8, r1: u8 },
    /// Per column, a `thickness`-row band centred on
    /// `mid_row + amplitude · sin(col / wavelength + phase + speed · t)`,
    /// where `t` is seconds since the keyframe began — amplitude in rows,
    /// wavelength in columns-per-radian, speed in radians/second.
    SineWave {
        amplitude: f32,
        wavelength: f32,
        speed: f32,
        phase: f32,
        thickness: u8,
    },
}

/// One keyframe of a tile script: while `from..to` (ticks, end-exclusive) is
/// active, the selected cells wear `kind`.
#[derive(Clone, Copy, PartialEq, Debug, Serialize, Deserialize)]
pub struct TileKeyframe {
    pub from: u32,
    pub to: u32,
    pub selector: TileSelector,
    pub kind: TileKind,
}

/// One arena's tile choreography on disk — the sibling of
/// [`crate::encounter::BossScoreFile`].
#[derive(Clone, PartialEq, Debug, Serialize, Deserialize)]
pub struct TileScriptFile {
    pub format: u32,
    pub arena: u8,
    pub difficulty: Difficulty,
    pub keyframes: Vec<TileKeyframe>,
}

impl TileScriptFile {
    /// Rejects a header the reader can't trust — same contract as
    /// [`crate::encounter::BossScoreFile::validate`].
    pub fn validate(&self, arena: Arena, difficulty: Difficulty) -> Result<(), ScoreIoError> {
        validate_header(self.format, self.arena, self.difficulty, arena, difficulty)
    }
}

/// Bookkeeping for one cell a [`TileScript`] currently holds: the scripted
/// `kind` it wears, and the `prior` kind it wore before the script claimed it —
/// restored when the script lets go, so a manually-flipped tile is never the
/// script's to undo.
#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub struct AppliedCell {
    pub kind: TileKind,
    pub prior: TileKind,
}

/// An arena root's live tile script: the keyframes driving its board, plus the
/// cells the script currently holds (with what they wore before, so the applier
/// can revert exactly what it touched — and nothing else). `dirty` marks
/// keyframes edited in the author tool since the last save/load; an unsaved
/// script is never overwritten by the score sync.
#[derive(Component, Default)]
pub struct TileScript {
    pub keyframes: Vec<TileKeyframe>,
    pub applied: HashMap<(u8, u8), AppliedCell>,
    pub dirty: bool,
}

/// The cells `selector` covers `ticks_in` ticks after its keyframe began.
/// Always in-bounds; out-of-range parts of a selection clamp away silently.
pub fn cells_at(selector: &TileSelector, ticks_in: u32) -> Vec<(u8, u8)> {
    let mut cells = Vec::new();
    match *selector {
        TileSelector::Cell { col, row } => {
            if (col as i32) <= MAX_COL && (row as i32) <= MAX_ROW {
                cells.push((col, row));
            }
        }
        TileSelector::Row { row } => {
            if (row as i32) <= MAX_ROW {
                cells.extend((0..GRID_W as u8).map(|col| (col, row)));
            }
        }
        TileSelector::Col { col } => {
            if (col as i32) <= MAX_COL {
                cells.extend((0..GRID_H as u8).map(|row| (col, row)));
            }
        }
        TileSelector::Rect { c0, r0, c1, r1 } => {
            let (lo_c, hi_c) = (c0.min(c1), c0.max(c1).min(MAX_COL as u8));
            let (lo_r, hi_r) = (r0.min(r1), r0.max(r1).min(MAX_ROW as u8));
            for col in lo_c..=hi_c {
                for row in lo_r..=hi_r {
                    cells.push((col, row));
                }
            }
        }
        TileSelector::SineWave {
            amplitude,
            wavelength,
            speed,
            phase,
            thickness,
        } => {
            let t = ticks_in as f32 / TICKS_PER_SECOND as f32;
            let mid = GRID_H.strict_sub(1) as f32 * 0.5;
            // Guard the division; a zero wavelength means "no horizontal variation".
            let freq = if wavelength.abs() > f32::EPSILON {
                1.0 / wavelength
            } else {
                0.0
            };
            let half_down = (thickness.max(1) as i32).strict_sub(1) / 2;
            let half_up = thickness.max(1) as i32 / 2;
            for col in 0..GRID_W {
                let centre = mid + amplitude * (col as f32 * freq + phase + speed * t).sin();
                let centre = centre.round() as i32;
                for row in centre.saturating_sub(half_down)..=centre.saturating_add(half_up) {
                    if (0..GRID_H).contains(&row) {
                        cells.push((col as u8, row as u8));
                    }
                }
            }
        }
    }
    cells
}

/// The full board delta at `tick`: every cell an active keyframe touches,
/// mapped to its kind. Later keyframes win overlaps (file order is paint
/// order). Cells absent from the map are `Normal` as far as the script cares.
pub fn desired(keyframes: &[TileKeyframe], tick: u32) -> HashMap<(u8, u8), TileKind> {
    let mut out = HashMap::new();
    for keyframe in keyframes {
        if (keyframe.from..keyframe.to).contains(&tick) {
            for cell in cells_at(&keyframe.selector, tick.strict_sub(keyframe.from)) {
                out.insert(cell, keyframe.kind);
            }
        }
    }
    out
}

#[cfg(test)]
mod tests {
    use super::*;

    fn frame(from: u32, to: u32, selector: TileSelector) -> TileKeyframe {
        TileKeyframe {
            from,
            to,
            selector,
            kind: TileKind::Lava,
        }
    }

    #[test]
    fn static_selectors_cover_what_they_say_and_clamp() {
        assert_eq!(
            cells_at(&TileSelector::Cell { col: 1, row: 5 }, 0),
            vec![(1, 5)]
        );
        assert!(cells_at(&TileSelector::Cell { col: 200, row: 5 }, 0).is_empty());
        assert_eq!(
            cells_at(&TileSelector::Row { row: 0 }, 0).len(),
            GRID_W as usize
        );
        assert_eq!(
            cells_at(&TileSelector::Col { col: 0 }, 0).len(),
            GRID_H as usize
        );
        // Reversed corners normalize; the overflowing edge clamps to the board.
        let rect = cells_at(
            &TileSelector::Rect {
                c0: 3,
                r0: 4,
                c1: 1,
                r1: 2,
            },
            0,
        );
        assert_eq!(rect.len(), 9);
        assert!(rect.contains(&(1, 2)) && rect.contains(&(3, 4)));
        let clamped = cells_at(
            &TileSelector::Rect {
                c0: 64,
                r0: 29,
                c1: 200,
                r1: 200,
            },
            0,
        );
        assert_eq!(clamped.len(), 4); // cols 64-65 × rows 29-30
    }

    #[test]
    fn sine_wave_stays_in_bounds_and_animates() {
        let wave = TileSelector::SineWave {
            amplitude: 40.0, // deliberately over-amplitude: must clamp, not panic
            wavelength: 8.0,
            speed: 2.0,
            phase: 0.0,
            thickness: 3,
        };
        for ticks_in in [0, 30, 60, 600, 7199] {
            for (col, row) in cells_at(&wave, ticks_in) {
                assert!((col as i32) <= MAX_COL && (row as i32) <= MAX_ROW);
            }
        }
        // A flat (amplitude 0, thickness 1) wave is exactly the middle row.
        let flat = TileSelector::SineWave {
            amplitude: 0.0,
            wavelength: 8.0,
            speed: 1.0,
            phase: 0.0,
            thickness: 1,
        };
        let cells = cells_at(&flat, 0);
        assert_eq!(cells.len(), GRID_W as usize);
        assert!(cells.iter().all(|&(_, row)| row == (GRID_H as u8 - 1) / 2));
        // Speed moves the band over time.
        let moving = TileSelector::SineWave {
            amplitude: 10.0,
            wavelength: 8.0,
            speed: 2.0,
            phase: 0.0,
            thickness: 1,
        };
        assert_ne!(cells_at(&moving, 0), cells_at(&moving, 60));
    }

    #[test]
    fn keyframe_ranges_are_end_exclusive_and_later_wins() {
        let cell = TileSelector::Cell { col: 2, row: 2 };
        let script = [frame(10, 20, cell)];
        assert!(desired(&script, 9).is_empty());
        assert_eq!(desired(&script, 10).len(), 1);
        assert_eq!(desired(&script, 19).len(), 1);
        assert!(desired(&script, 20).is_empty());

        // Overlap: the later keyframe's kind wins the shared cell.
        let layered = [
            frame(0, 100, cell),
            TileKeyframe {
                from: 0,
                to: 100,
                selector: cell,
                kind: TileKind::Normal,
            },
        ];
        assert_eq!(desired(&layered, 50)[&(2, 2)], TileKind::Normal);
    }

    #[test]
    fn readers_reject_foreign_headers() {
        let file = TileScriptFile {
            format: crate::encounter::SCORE_FORMAT,
            arena: Arena::Hunter.index() as u8,
            difficulty: Difficulty::Heroic,
            keyframes: vec![],
        };
        assert!(file.validate(Arena::Hunter, Difficulty::Heroic).is_ok());
        assert!(file.validate(Arena::Bard, Difficulty::Heroic).is_err());
        assert!(file.validate(Arena::Hunter, Difficulty::Normal).is_err());
        let newer = TileScriptFile {
            format: crate::encounter::SCORE_FORMAT + 1,
            ..file
        };
        assert!(newer.validate(Arena::Hunter, Difficulty::Heroic).is_err());
    }

    #[test]
    fn tile_script_file_round_trips_through_ron() {
        let file = TileScriptFile {
            format: crate::encounter::SCORE_FORMAT,
            arena: 0,
            difficulty: Difficulty::Normal,
            keyframes: vec![
                frame(
                    0,
                    600,
                    TileSelector::Rect {
                        c0: 1,
                        r0: 1,
                        c1: 4,
                        r1: 4,
                    },
                ),
                frame(
                    600,
                    1200,
                    TileSelector::SineWave {
                        amplitude: 6.0,
                        wavelength: 10.0,
                        speed: 1.5,
                        phase: 0.5,
                        thickness: 2,
                    },
                ),
            ],
        };
        let text = ron::ser::to_string_pretty(&file, ron::ser::PrettyConfig::default()).unwrap();
        let back: TileScriptFile = ron::from_str(&text).unwrap();
        assert_eq!(back, file);
    }
}
