//! `cargo xtask choreograph` — generate the title-screen circle choreography.
//!
//! Reads the detected beat grid (`assets/title/beats.ron`) and produces a
//! **deterministic** 2-minute flow-through: ~40 circles enter from all four
//! edges on the beat, weave across a grid, and exit the far side — and **no two
//! ever share a cell**. The result is written as a versioned RON score
//! (`assets/title/choreography.vNNNN.ron`), which the title scene plays back.
//!
//! How no-overlap is guaranteed (4-directional, reservation-based):
//! - movement happens once per beat; each cell is claimed by at most one agent
//!   per beat (a `claim` map), so two circles can't land on the same cell;
//! - an agent may only move into a cell that is empty *or* whose occupant has
//!   already vacated this beat — never into a still-occupied cell — which also
//!   makes head-on swaps impossible;
//! - staying put is always legal, so the scheme never deadlocks.
//! After generating, [`verify`] reconstructs every position at every beat and
//! asserts both invariants exhaustively — the score is proven, not assumed.

use std::collections::HashMap;

use anyhow::{Context, Result, bail};

use crate::score::{Agent, Choreography, Step};

const GRID_W: i32 = 32;
const GRID_H: i32 = 18;
const N_AGENTS: usize = 40;
/// Nonzero xorshift seed (golden ratio) — fixed, so the score is reproducible.
const SEED: u64 = 0x9E37_79B9_7F4A_7C15;
/// An agent must travel at least this many beats before it's allowed to exit
/// (keeps it from leaving the instant it spawns on a corner).
const MIN_TRAVEL: usize = 3;

/// Deterministic xorshift64* RNG — no `rand` dependency, fully reproducible.
struct Rng(u64);

impl Rng {
    fn next_u64(&mut self) -> u64 {
        let mut x = self.0;
        x ^= x << 13;
        x ^= x >> 7;
        x ^= x << 17;
        self.0 = x;
        x.wrapping_mul(0x2545_F491_4F6C_DD1D)
    }
    fn below(&mut self, n: usize) -> usize {
        (self.next_u64() % n as u64) as usize
    }
    fn f01(&mut self) -> f64 {
        (self.next_u64() >> 11) as f64 / (1u64 << 53) as f64
    }
}

type Cell = (i32, i32);

/// The four edges a circle can enter or exit by.
fn on_edge(cell: Cell, edge: u8) -> bool {
    match edge {
        0 => cell.1 == GRID_H - 1, // top
        1 => cell.1 == 0,          // bottom
        2 => cell.0 == 0,          // left
        _ => cell.0 == GRID_W - 1, // right
    }
}

/// An interior cell on `edge` at parameter `along` (kept off the corners).
fn edge_cell(edge: u8, along: i32) -> Cell {
    match edge {
        0 => (along, GRID_H - 1),
        1 => (along, 0),
        2 => (0, along),
        _ => (GRID_W - 1, along),
    }
}

/// The valid interior range of `along` for an edge (2 cells in from each corner).
fn edge_span(edge: u8) -> (i32, i32) {
    match edge {
        0 | 1 => (2, GRID_W - 3),
        _ => (2, GRID_H - 3),
    }
}

/// Move preference order from `from` toward `target`: the goal directions first
/// (momentum + a per-beat weave decide their order), then perpendicular wander
/// directions (shuffled for swirl), then "stay" as the always-legal fallback.
fn move_order(
    from: Cell,
    target: Cell,
    last: Cell,
    weave_x_first: bool,
    rng: &mut Rng,
) -> Vec<Cell> {
    let gx = (target.0 - from.0).signum();
    let gy = (target.1 - from.1).signum();
    let mut goals: Vec<Cell> = Vec::new();
    if weave_x_first {
        if gx != 0 {
            goals.push((gx, 0));
        }
        if gy != 0 {
            goals.push((0, gy));
        }
    } else {
        if gy != 0 {
            goals.push((0, gy));
        }
        if gx != 0 {
            goals.push((gx, 0));
        }
    }
    // Momentum: keep gliding the same way when it still serves the goal.
    if goals.contains(&last) {
        goals.retain(|&d| d != last);
        goals.insert(0, last);
    }
    // Wander directions (shuffled) — used only when the goal dirs are blocked,
    // giving organic avoidance/swirl instead of everyone picking the same side.
    let mut wander = [(1, 0), (-1, 0), (0, 1), (0, -1)];
    for i in (1..wander.len()).rev() {
        wander.swap(i, rng.below(i + 1));
    }
    let mut order = goals;
    for d in wander {
        if !order.contains(&d) {
            order.push(d);
        }
    }
    order.push((0, 0));
    order
}

/// Mutable per-agent state during the simulation.
struct Sim {
    spawn_beat: usize,
    despawn_beat: usize,
    start: Cell,
    steps: Vec<(usize, i32, i32)>, // (beat the move completes on, dx, dy)
    cur: Option<Cell>,
    last_dir: Cell,
    placed: bool,
    // schedule (fixed up front)
    desired_spawn: usize,
    entry_edge: u8,
    entry_along: i32,
    exit_edge: u8,
    target: Cell,
}

pub fn run() -> Result<()> {
    let txt = std::fs::read_to_string("assets/title/beats.ron")
        .context("read assets/title/beats.ron — run `cargo xtask beats` first")?;
    let beats: crate::score::Beats = ron::from_str(&txt).context("parse beats.ron")?;
    let n_beats = beats.beat_ticks.len();
    if n_beats < 32 {
        bail!("only {n_beats} beats detected — too few to choreograph");
    }
    let cycle_ticks = (beats.duration_secs * 60.0).round() as u32;

    let mut rng = Rng(SEED);
    let mut agents = schedule(&mut rng, n_beats);
    simulate(&mut agents, n_beats, &mut rng);
    verify(&agents, n_beats);

    // Convert the beat-indexed sim into a tick-stamped RON score.
    let beat_tick = |b: usize| beats.beat_ticks[b.min(n_beats - 1)];
    let out_agents: Vec<Agent> = agents
        .iter()
        .filter(|a| a.placed)
        .map(|a| Agent {
            spawn_tick: beat_tick(a.spawn_beat),
            despawn_tick: if a.despawn_beat >= n_beats {
                cycle_ticks
            } else {
                beat_tick(a.despawn_beat)
            },
            start: [a.start.0, a.start.1],
            steps: a
                .steps
                .iter()
                .map(|&(b, dx, dy)| Step {
                    tick: beat_tick(b),
                    dx,
                    dy,
                })
                .collect(),
        })
        .collect();

    let choreo = Choreography {
        grid_w: GRID_W as u32,
        grid_h: GRID_H as u32,
        cycle_ticks,
        agents: out_agents,
    };

    let version = next_version("assets/title", "choreography")?;
    let out = format!("assets/title/choreography.v{version:04}.ron");
    let ron = ron::ser::to_string_pretty(&choreo, ron::ser::PrettyConfig::default())
        .context("serialize choreography")?;
    std::fs::write(&out, &ron).with_context(|| format!("write {out}"))?;
    // Also overwrite a stable copy: the runtime `include_str!`s this (web-safe,
    // no dir listing), while the versioned file is the rollback history.
    std::fs::write("assets/title/choreography.latest.ron", &ron)
        .context("write choreography.latest.ron")?;

    report(&agents, &choreo, n_beats, &out);
    Ok(())
}

/// Fixed entry schedule: spread 40 agents across the WHOLE cycle (beat 0 → end),
/// each assigned an entry edge + an exit on a *different* edge. Starting at beat
/// 0 means circles are on screen from the first frame; spreading to the end means
/// late entrants (who can't finish crossing) linger to the cycle boundary and
/// fill the tail — so the field never empties through the loop wrap.
fn schedule(rng: &mut Rng, n_beats: usize) -> Vec<Sim> {
    let span = n_beats.saturating_sub(1).max(1);
    let mut agents = Vec::with_capacity(N_AGENTS);
    for k in 0..N_AGENTS {
        // Even spread from beat 0, with a small jitter so entries still scatter.
        // The first agent is pinned to beat 0 so the very first frame (and every
        // loop wrap) already has a circle on screen — no blank white sheet.
        let jitter = rng.below(2);
        let desired_spawn = if k == 0 {
            0
        } else {
            (k * span / N_AGENTS + jitter).min(span)
        };

        let entry_edge = (rng.below(4)) as u8;
        let (lo, hi) = edge_span(entry_edge);
        let entry_along = lo + rng.below((hi - lo + 1) as usize) as i32;

        // Exit on a different edge → cross-currents.
        let mut exit_edge = (rng.below(4)) as u8;
        if exit_edge == entry_edge {
            exit_edge = (exit_edge + 1 + rng.below(3) as u8) % 4;
        }
        let (elo, ehi) = edge_span(exit_edge);
        let target = edge_cell(exit_edge, elo + rng.below((ehi - elo + 1) as usize) as i32);

        agents.push(Sim {
            spawn_beat: usize::MAX,
            despawn_beat: n_beats,
            start: (0, 0),
            steps: Vec::new(),
            cur: None,
            last_dir: (0, 0),
            placed: false,
            desired_spawn,
            entry_edge,
            entry_along,
            exit_edge,
            target,
        });
    }
    agents
}

/// Beat-by-beat cooperative simulation (see module docs for the invariants).
fn simulate(agents: &mut [Sim], n_beats: usize, rng: &mut Rng) {
    for b in 0..n_beats {
        // Positions occupied at beat `b`.
        let mut occ: HashMap<Cell, usize> = HashMap::new();
        for (id, a) in agents.iter().enumerate() {
            if let Some(c) = a.cur {
                occ.insert(c, id);
            }
        }

        // Spawn anything due (retries each beat until a free entry cell opens).
        for id in 0..agents.len() {
            if agents[id].placed || agents[id].desired_spawn > b {
                continue;
            }
            let edge = agents[id].entry_edge;
            let (lo, hi) = edge_span(edge);
            let pref = agents[id].entry_along;
            // Try the preferred cell, then fan outwards along the edge.
            let mut chosen = None;
            for d in 0..=(hi - lo) {
                for s in [d, -d] {
                    let along = pref + s;
                    if along < lo || along > hi {
                        continue;
                    }
                    let cell = edge_cell(edge, along);
                    if !occ.contains_key(&cell) {
                        chosen = Some(cell);
                        break;
                    }
                }
                if chosen.is_some() {
                    break;
                }
            }
            if let Some(cell) = chosen {
                agents[id].cur = Some(cell);
                agents[id].start = cell;
                agents[id].spawn_beat = b;
                agents[id].placed = true;
                occ.insert(cell, id);
            }
        }

        if b + 1 >= n_beats {
            continue; // last beat: no further moves
        }

        // Plan moves into beat b+1.
        let mut claim: HashMap<Cell, usize> = HashMap::new();
        let mut moved_from: HashMap<usize, Cell> = HashMap::new();
        let mut move_to: HashMap<usize, Cell> = HashMap::new();
        let mut next: Vec<Option<Cell>> = agents.iter().map(|a| a.cur).collect();

        for id in 0..agents.len() {
            let Some(from) = agents[id].cur else {
                continue;
            };
            // Exit once we've travelled a bit and reached the exit edge.
            if b > agents[id].spawn_beat + MIN_TRAVEL && on_edge(from, agents[id].exit_edge) {
                agents[id].despawn_beat = b + 1;
                next[id] = None;
                continue;
            }
            let weave_x_first = rng.f01() < 0.5;
            let order = move_order(
                from,
                agents[id].target,
                agents[id].last_dir,
                weave_x_first,
                rng,
            );
            let mut picked = (0, 0, from);
            for (dx, dy) in order {
                if (dx, dy) == (0, 0) {
                    picked = (0, 0, from); // stay — always legal
                    break;
                }
                let to = (from.0 + dx, from.1 + dy);
                if to.0 < 0 || to.0 >= GRID_W || to.1 < 0 || to.1 >= GRID_H {
                    continue;
                }
                if claim.contains_key(&to) {
                    continue;
                }
                // Only enter a beat-`b`-occupied cell if its occupant vacated.
                if let Some(&z) = occ.get(&to) {
                    match move_to.get(&z) {
                        Some(&zt) if zt != to => {}
                        _ => continue,
                    }
                }
                // Belt-and-suspenders head-on swap guard.
                let swap = move_to
                    .iter()
                    .any(|(w, &t)| t == from && moved_from.get(w) == Some(&to));
                if swap {
                    continue;
                }
                picked = (dx, dy, to);
                break;
            }
            let (dx, dy, to) = picked;
            claim.insert(to, id);
            moved_from.insert(id, from);
            move_to.insert(id, to);
            next[id] = Some(to);
            if (dx, dy) != (0, 0) {
                agents[id].last_dir = (dx, dy);
                agents[id].steps.push((b + 1, dx, dy));
            }
        }

        for (id, a) in agents.iter_mut().enumerate() {
            a.cur = next[id];
        }
    }
}

/// Reconstruct every position at every beat and assert the two invariants —
/// the exhaustive proof that the score never overlaps.
fn verify(agents: &[Sim], n_beats: usize) {
    let pos_at = |a: &Sim, j: usize| -> Option<Cell> {
        if !a.placed || j < a.spawn_beat || j >= a.despawn_beat {
            return None;
        }
        let mut c = a.start;
        for &(beat, dx, dy) in &a.steps {
            if beat <= j {
                c.0 += dx;
                c.1 += dy;
            }
        }
        Some(c)
    };

    for j in 0..n_beats {
        let mut seen: HashMap<Cell, usize> = HashMap::new();
        for (id, a) in agents.iter().enumerate() {
            if let Some(c) = pos_at(a, j) {
                assert!(
                    c.0 >= 0 && c.0 < GRID_W && c.1 >= 0 && c.1 < GRID_H,
                    "agent {id} out of bounds {c:?} at beat {j}"
                );
                if let Some(&other) = seen.get(&c) {
                    panic!("OVERLAP: agents {id} and {other} both on {c:?} at beat {j}");
                }
                seen.insert(c, id);
            }
        }
        // No blank frame: at least one circle is on screen on every beat, so the
        // title never shows an empty white sheet (including across the loop wrap).
        assert!(!seen.is_empty(), "BLANK: no circle alive at beat {j}");
    }
    // Head-on swap check across consecutive beats.
    for j in 0..n_beats.saturating_sub(1) {
        for (p, ap) in agents.iter().enumerate() {
            let (Some(px), Some(py)) = (pos_at(ap, j), pos_at(ap, j + 1)) else {
                continue;
            };
            if px == py {
                continue;
            }
            for (q, aq) in agents.iter().enumerate().skip(p + 1) {
                let (Some(qx), Some(qy)) = (pos_at(aq, j), pos_at(aq, j + 1)) else {
                    continue;
                };
                if px == qy && py == qx {
                    panic!(
                        "SWAP: agents {p} and {q} cross between beats {j} and {}",
                        j + 1
                    );
                }
            }
        }
    }
}

/// Next free `prefix.vNNNN.ron` version number in `dir` (mirrors the encounter /
/// layer score convention: highest version wins, roll back by deleting).
fn next_version(dir: &str, prefix: &str) -> Result<u32> {
    std::fs::create_dir_all(dir).with_context(|| format!("create {dir}"))?;
    let mut max = 0u32;
    for entry in std::fs::read_dir(dir)? {
        let name = entry?.file_name();
        let name = name.to_string_lossy();
        if let Some(rest) = name.strip_prefix(&format!("{prefix}.v")) {
            if let Some(num) = rest.strip_suffix(".ron") {
                if let Ok(v) = num.parse::<u32>() {
                    max = max.max(v);
                }
            }
        }
    }
    Ok(max + 1)
}

/// Print the stats that matter without eyes on the screen: BPM-locked beats,
/// total/peak concurrency, entries per edge, and that no-overlap is proven.
fn report(agents: &[Sim], choreo: &Choreography, n_beats: usize, out: &str) {
    let placed = agents.iter().filter(|a| a.placed).count();
    // Concurrency over the beats — peak AND the floor (the floor proves the
    // field never empties / no blank title screen).
    let mut peak = 0usize;
    let mut floor = usize::MAX;
    for j in 0..n_beats {
        let live = agents
            .iter()
            .filter(|a| a.placed && j >= a.spawn_beat && j < a.despawn_beat)
            .count();
        peak = peak.max(live);
        floor = floor.min(live);
    }
    let mut by_edge = [0usize; 4];
    for a in agents.iter().filter(|a| a.placed) {
        by_edge[a.entry_edge as usize] += 1;
    }
    let total_steps: usize = choreo.agents.iter().map(|a| a.steps.len()).sum();
    let avg_life = agents
        .iter()
        .filter(|a| a.placed)
        .map(|a| a.despawn_beat.min(n_beats) - a.spawn_beat)
        .sum::<usize>() as f64
        / placed.max(1) as f64;

    println!(
        "grid {}×{}  cycle {} ticks",
        choreo.grid_w, choreo.grid_h, choreo.cycle_ticks
    );
    println!(
        "agents placed {placed}/{N_AGENTS}  on screen {floor}–{peak}  avg life {avg_life:.0} beats"
    );
    println!(
        "entries  top {}  bottom {}  left {}  right {}",
        by_edge[0], by_edge[1], by_edge[2], by_edge[3]
    );
    println!("total move steps {total_steps}  — no-overlap VERIFIED across {n_beats} beats");
    println!("wrote {out}");
}
