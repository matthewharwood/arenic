# Arena Sky-Swarm (under-floor)

The **sky-swarm** is the *discrete* moving layer of each arena's atmosphere — a
small, drifting cloud of **motes** (the "fauna / probes in the sky") that adds
texture and depth **without ever interrupting play**. It rides UNDER the
translucent liquid-glass floor, so it goes *around and under* the board but never
*through* it, seen glowing softly up through the glass. The full atmosphere stack,
camera → sky, is:

> camera → **foreground haze plane** → **liquid-glass floor** →
> board / boss / flora-fauna → **sky-swarm (under the floor)** → **skybox**

Built in `crates/arenic_storybook/src/foreground.rs` (the swarm section, beside
the cloud/fog shader); driven by the `ForegroundPlugin`. This is the discrete
counterpart to the `arena_fog.wgsl` haze planes. The per-arena swarm row lives in
the single `ArenaSpec` table (`arena.rs`), beside its theme / voices / boss / props.

## Canonical mapping (2026-06-04 remap)

Keyed by **class** (the stable join). Story names in `arenic_storybook` are
canonical and `_docs/arena_model.go` is regenerated to this grid; the Linear
issues (ARE-18…26) are still pending reconciliation.

| idx | Class | Arena | Theme |
|---|---|---|---|
| 0 | Hunter | Labyrinth | Tokyo Night |
| 1 | Guildmaster | Guild House | Coffee |
| 2 | Cardinal | Sanctum | Luxury |
| 3 | Forager | Mountain | Forest |
| 4 | Warrior | Bastion | Gruvbox Dark |
| 5 | Thief | Pawnshop | Ayu Dark |
| 6 | Alchemist | Crucible | Abyss |
| 7 | Merchant | Casino | Rosé Pine |
| 8 | Bard | Gala | Synthwave |

## The grammar (cohesion — why the nine read as one world)

- **One system, data-driven.** All arenas use the same `SwarmMember` +
  `swarm_offset` driver (mirrors the hollow-light pattern); each arena's
  `SwarmSpec { motion, mote, count, scale }` is one field of its `ArenaSpec` row.
- **Shared silhouette families.** `Mote::{Spark, Flake, Dart, Bubble}` — a small
  vocabulary reused across arenas (like the model's moth/beetle/newt families).
- **Shared placement + scale.** Small (≈0.06–0.11 u), translucent (α 0.55) +
  lightly emissive (blooms via the stage's HDR+Bloom), **sparse** (10–16), in a
  **low ring UNDER the floor at `z = -1.4`** — riding `z ∈ [-2.3, -0.5]`, always
  below the floor plane `z = -0.02` (a compile-time invariant) — biased to the
  board's outer band (`r ∈ [0.65, 1.05]` of a 7.5 × 3.4 ellipse) so it glows up
  through the liquid glass without occluding the boss. Deterministic golden-angle
  spread + per-index phase stagger (no RNG) → a "meaningful but not distracting" order.
- **Theme colour.** Every swarm tints to its arena's `theme.primary` accent, so
  it re-tones on theme switch and is per-arena distinct because the themes are.

## The distance (per-arena uniqueness — carried by motion + cadence + colour)

| Arena · Class | Object | `SwarmMotion` | Motion / cadence |
|---|---|---|---|
| Labyrinth · Hunter | scout **darts** (`Dart`) | `Patrol` | straight glides (tanh-sharpened) + slow turns, patrolling sightlines |
| Guild House · Guildmaster | hearth **embers** (`Spark`) | `HearthRise` | gentle warm updraft + sway — the calmest swarm (the safe home) |
| Sanctum · Cardinal | gilt-**leaf** (`Flake`) | `PendulumFall` | near-vertical pendulum descent, reverent |
| Mountain · Forager | **spores** (`Spark`) | `GustDrift` | wind-borne wander with a slow secondary gust |
| Bastion · Warrior | forge **cinders** (`Flake`) | `UpdraftChurn` | stall-flip-rise on the heat — net upward churn |
| Pawnshop · Thief | bats / coin-**flakes** (`Dart`) | `FurtiveDart` | furtive dart-pause-dart (tanh holds), irregular |
| Crucible · Alchemist | **bubbles** (`Bubble`) | `OpposingDrift` | half rise / half fall (per-index z-sign) — opposing crawl |
| Casino · Merchant | tumbling **coins** (`Flake`) | `TumbleArc` | end-over-end tumble (axis spin) + tight mechanical arc |
| Gala · Bard | **confetti** (`Flake`) | `BeatDrift` | drift + a synchronized on-beat (`sin⁴`) upward pluck |

**Non-distracting guarantees:** under the floor (a separated layer seen softly
through the glass), small + translucent + sparse, slow (global `0.5×` time
factor), outer-ring biased (boss zone clear), `NotShadowCaster`/`NotShadowReceiver`.

## Tuning knobs

- Per arena: the `swarm` field of the arena's `ArenaSpec` row in `arena.rs` —
  `SwarmSpec { motion, mote, count, scale }`.
- Per motion: the `swarm_offset()` arm.
- Global: the `SWARM_*` consts in `foreground.rs` — `SWARM_TIME_SCALE` (0.5×),
  `SWARM_RING_X/Y`, `SWARM_Z = -1.4`, `SWARM_AMP` — plus α `0.55` + emissive
  `1.6×` (`swarm_material`).

## Future

- Optional Blender silhouettes (a real moth/bat/coin/leaf) can replace the
  primitive `Mote` meshes; the system is mesh-agnostic.
- Per-arena 2nd accent colour (the model rations two per arena) — currently every
  swarm uses `theme.primary`.
- Reconcile the Linear ARE-18…26 issues to the canonical remap (`arena_model.go`
  and the `ArenaSpec` table are already on it).
