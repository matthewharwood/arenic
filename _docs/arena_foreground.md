# Arena Foreground Sky-Swarm

The **foreground** is the nearest moving layer in each arena's atmosphere stack
(camera → **foreground swarm** → near-haze plane → board/boss/flora-fauna →
translucent floor → skybox). It is a small, drifting cloud of **discrete motes**
— the "fauna / probes in the sky" — that adds texture and depth between the board
and the camera **without ever interrupting play**.

Built in `crates/arenic_storybook/src/foreground.rs` (the swarm section, beside
the cloud/fog shader); driven by the `ForegroundPlugin`. This is the discrete
counterpart to the `arena_fog.wgsl` haze planes.

## Canonical mapping (2026-06-04 remap)

Keyed by **class** (the stable join). Story names in `arenic_storybook` are
canonical; `_docs/arena_model.go` + the Linear issues are still on the **old**
pairings and remain pending a full regen.

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
  `swarm_offset` driver (mirrors the hollow-light pattern): per-arena rows of
  `(motion, silhouette, count, scale)`.
- **Shared silhouette families.** `Mote::{Spark, Flake, Dart, Bubble}` — a small
  vocabulary reused across arenas (like the model's moth/beetle/newt families).
- **Shared placement + scale.** Small (≈0.06–0.11 u), translucent (α 0.55) +
  lightly emissive (blooms via the stage's HDR+Bloom), **sparse** (10–16), in a
  **high sky ring at z ≈ 5.5** — far above the boss (z ≤ 2.5) and biased to the
  board's outer band (`r ∈ [0.65, 1.05]` of a 7.5 × 3.4 ellipse) so the central
  play field stays clear. Deterministic golden-angle spread + per-index phase
  stagger (no RNG) → a "meaningful but not distracting" order.
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

**Non-distracting guarantees:** high z (separated layer), small + translucent +
sparse, slow (global `0.5×` time factor), outer-ring biased (boss zone clear),
`NotShadowCaster`/`NotShadowReceiver` (no shadows on the board).

## Tuning knobs

- Per arena: the `arena_swarm()` row — `(motion, Mote, count, scale)`.
- Per motion: `swarm_offset()` arm.
- Global: `0.5×` time factor (`animate_swarm`), ring radii + `z = 5.5` + `amp`
  (`respawn_swarm`), α `0.55` + emissive `1.6×` (`swarm_material`).

## Future

- Optional Blender silhouettes (a real moth/bat/coin/leaf) can replace the
  primitive `Mote` meshes; the system is mesh-agnostic.
- Per-arena 2nd accent colour (the model rations two per arena) — currently every
  swarm uses `theme.primary`.
- Reconcile `_docs/arena_model.go` + Linear ARE-18…26 to the canonical remap and
  fold these swarm specs into each arena's `Foreground` axis.
