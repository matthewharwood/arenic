# The Authoring Suite — layers, dope sheet, entity browser

The design contract for the layer-based authoring suite (ARE-37…46), distilled
from a four-track research pass (2026-06-10): the dope sheets of After Effects,
Blender, Houdini, and Rive; the `@pierre/trees` (trees.software) interaction
model read from source; the verified Bevy 0.18.1 capability surface; and the
arenic codebase seam map. **The bar is explicit: the dope sheet must exceed
After Effects.** Everything here builds on the author mode (`just author`) and
the sheet-music timeline (`_docs/RULEBOOK.md`).

Legend for interaction tables: **[T]** = table-stakes (industry muscle memory,
keep verbatim) · **[D]** = differentiator (where we beat AE) · source in
parentheses.

---

## 1. The layer model

### 1.1 Shape

Authoring is **layer-based per (arena, difficulty)**. A `LayerStack` is an
ordered list of layers; the PANEL shows the top of the stack first (AE
convention).

```rust
LayerStack { layers: Vec<Layer> }            // index 0 = bottom, last = top
Layer {
    id: LayerId,                              // stable within the stack's file
    name: String,                             // "Boss", "Tiles — waves", "Minion A"
    kind: LayerKind,
    muted: bool,                              // excluded from fold
    solo: bool,                               // if any solo: non-solo = muted
    locked: bool,                             // UI-only: keys unselectable
    effects: Vec<EffectTrack>,                // §4
}
LayerKind {
    Boss(Recording),                          // the boss staff (one per stack, by convention not by type)
    Minion { archetype, spawn_tick, despawn_tick, spawn_tile, recording },
    Tiles(Vec<TileKeyframe>),                 // N instances legal — partitions, waves, …
}
```

**Layer instances** are first-class: duplicate a tile layer, cut it, author a
second one. A tile layer "partition" is nothing special — it's a layer whose
keyframes happen to cover a region (the selector vocabulary already supports
Cell/Row/Col/Rect/SineWave).

### 1.2 Invariants (locked)

1. **Fold-at-start:** the game (and author preview) folds all published,
   unmuted layers into ONE master `ArenaTimeline` + one merged tile schedule at
   load/cycle-restart. Playback never consults layers — it replays the single
   folded staff, exactly as today.
2. **Conflict rule — top wins:** fold applies layers bottom→top; where two tile
   layers claim the same cell at the same tick, the LATER-applied (higher)
   layer wins. This is AE stacking semantics and lifts the existing
   `desired()` "later keyframes win" rule to layer granularity. Deterministic;
   no tie-breaking heuristics.
3. **Tick quantization:** every key, event, spawn, and mark lives on a 60 Hz
   tick. There is no sub-tick anything (Rive's "Snap Keys interval" as a hard
   invariant).
4. **Draft vs published:** edits mutate the in-memory DRAFT stack and re-fold
   the live preview. **Publish** (`W`) writes the whole stack as
   `layers.vN+1.ron` — atomic whole-stack versioning, preserving the
   delete-the-newest-file rollback story and the tandem author↔game loop.
   Per-layer dirty = draft differs from the published baseline (the
   saved-baseline comparison pattern from `TileScript`).
5. **Effects are presentation:** effect tracks never alter recordings or sim
   state (§4). Removing an effect restores pristine visuals.

### 1.3 Files

One `assets/encounters/<arena>/<difficulty>/layers.vNNNN.ron` per stack
(SCORE_FORMAT bump; header carries format/arena/difficulty exactly like
today's files and is validated on read). Readers fall back to the legacy
`boss.vNNNN.ron` + `tiles.vNNNN.ron` pair (existing encounters keep working);
writers emit only the layered format. The `Action` event enum stays
**monomorphic** — richer payloads (e.g. `aim: Option<IVec2>`) extend the one
enum, never fork it (ARE-46).

### 1.4 Entity lifetime (the minion answer)

`GhostEvent.ghost` keys the master timeline by `Entity`. Minions therefore
**pre-spawn at fold time, hidden**, and an `ActiveWindow { spawn_tick,
despawn_tick }` drives `Visibility` + activity from the arena clock. No
mid-cycle entity churn: ids stay valid for the whole cycle, wrap resets
visibility, replay stays deterministic.

---

## 2. The dope sheet

Bottom panel of the author window (pop-out capable, §5). Three vertical zones:
**channel region** (rows: names + toggles + sliders) · **key area** (strips,
pills, playhead) · **ruler + zoom-scrollbar** on top.

### 2.1 Structure

- Hierarchy: **Summary row** (union of all keys — Blender) → layer rows →
  property channels (effect tracks) [T]. Layer rows show Rive-style rolled-up
  **group pills**; dragging a group pill moves all child keys at that tick [D].
- Recording strips render as dimmed spans (NLA-style); tile layers as
  keyframe-density strips.
- Per-row toggles, Blender-NLA semantics [T]: **Mute** (eye; dotted strip
  outline), **Solo** (star; additive multi-solo), **Lock** (padlock; dimmed,
  unselectable). Row-focused keys: `V` mute, `Shift+S` solo, `Ctrl+L` lock.
- **Dense-key degradation** [D]: >1 key per ~3 px on a row → heat-strip
  instead of individual pills (Houdini's #1 complaint is the anti-goal).
- Channel **value sliders that auto-key on drag** (Blender Show Sliders) [D].
- Keyframe **tags** (Blender types) [D]: Key / Breakdown / Hold / Extreme,
  visually distinct; `R` cycles. Grey bar between identical keys = held value;
  a tinted segment line marks non-linear interpolation — easing is visible
  WITHOUT any graph editor.

### 2.2 Ruler & playhead

- Adaptive unit ladder for 7,200 ticks @60 Hz: tick → ¼ s (15) → 1 s → 5 s →
  15 s → 1 min. Largest unit with ≥ ~80 px spacing renders labeled majors, one
  level below as minors; minors fade continuously while zooming (no popping).
  Labels `M:SS+tt` in DM Mono; toggle to raw ticks. Boss-phase boundaries
  (`PHASE_TICKS`) ride a thin marker lane under the ruler — the "BPM grid"
  request, repurposed for phases [D].
- **Playhead = 1 px line** with a small grabber above the ruler only — never a
  fat box occluding keys [D]. Click/drag in the ruler scrubs live (re-derives
  ghosts/tiles/effects via `timeline::seek_window`); `Shift` while scrubbing
  snaps to keys/markers [T]. `Ctrl+G` go-to accepts `1:23+45`, `5025`, `+120`,
  `-2s` [D]. Pending un-committed recording past the playhead tints it orange
  (Houdini's pending-change language) [D].

### 2.3 Zoom / pan / view

| Input | Behavior |
|---|---|
| Wheel | horizontal pan (timelines are horizontal documents) [T] |
| `Ctrl`+wheel | zoom centered on the cursor's tick [T] |
| `Shift`+wheel | vertical row scroll [T] |
| Middle-drag | freeform pan (Blender) [T] |
| Zoom-scrollbar above ruler | two grabbers: drag body = pan, drag grabber = zoom (Rive) [D] |
| `=` / `-` | zoom at playhead [T] |
| `\` | toggle last-zoom ↔ full 2 min (AE's beloved `;`) [T] |
| `A` / `F` | frame all / frame selection [T] |
| Clamps | max-in 1 tick ≥ ~8 px; max-out full cycle |

### 2.4 Selection grammar

- Click / `Shift`-add / `Ctrl`-toggle; box select; click empty = deselect [T].
- `Alt+B` **time-range select**: everything in the span across all visible
  rows (Blender) [D]. `Shift`-drag adds **discontiguous ranges** (Houdini) [D].
- Column ops: `Ctrl+K` = all keys at playhead tick; `K` = columns of selected
  keys (Blender) [D]. `Ctrl+A` / `Alt+A` / `Ctrl+I` all/none/invert [T].

### 2.5 Edit grammar (modal, Esc cancels)

| Key | Operation |
|---|---|
| `G` (+drag/arrows/digits) | move keys; numeric entry `G 12 ⏎` = +12 ticks (Blender modal + numeric) [D] |
| `S` | scale selection around playhead, time-axis only (Blender) [D] |
| `Shift+D` | duplicate selection into move-mode [T] |
| `X` / `Delete` | delete [T] |
| `Alt+←/→` | nudge ±1 tick; `+Shift` ±15 (AE verbatim) [T] |
| `W` | **ripple**: line at cursor; drag moves all keys after it; `Alt` = before (Houdini — AE has nothing like it) [D] |
| `Ctrl+Shift+D` | split recording strip at playhead (AE) [T] |
| `Shift+T` | **slide**: stretch one key-range, compress the neighbor — total duration constant (Blender; perfect fit for a fixed 2-min cycle) [D] |
| `Ctrl+C/X/V` | copy/cut/paste at playhead; `Ctrl+Shift+V` **visual paste** — ghost preview, place, ⏎ commit (Houdini) [D] |

Drag/snap matrix: plain drag snaps to **ticks always**; `Shift` = magnet to
playhead/markers/keys/loop edges (AE); `Ctrl` = ¼-second (15-tick) coarse
snap; `Alt`-drag on a selection edge scales the group in place (Rive), release
snaps to ticks (Houdini). Same-tick collisions auto-merge [D].

### 2.6 Transport & navigation

`Space` play/pause [T] · `J`/`K` previous/next key (AE) [T] · `←/→` ±1 tick,
`Shift` ±15 [T] · `Home`/`End` cycle start/end [T] · `B`/`N` loop-region
in/out at playhead (AE work area) [T] · `P`-drag loop region, `Alt+P` clear,
`Ctrl+Alt+P` loop-from-selection (Blender preview range) [D] · `M` marker,
`Ctrl+←/→` jump markers [T] · `1–9` jump to phase bookmark [D] · `Tab` /
`Shift+Tab` expand/collapse [T] · `U` reveal-animated + Only-Show-Selected
filter (the 50-minion cure) [D].

### 2.7 Easing UI — where we beat AE outright

1. **Inline interpolation panel** (`T`), Rive-style: docked beside the
   timeline, mini value-curve of the selection, Linear/Cubic/Hold buttons +
   draggable handles. **No separate graph-editor mode, no speed-vs-value
   duality** — AE's #1 beginner wall, deleted [D].
2. `F9` / `Shift+F9` / `Ctrl+Shift+F9` ease family (AE muscle memory,
   verbatim) [T]; `H` hold (one key, not AE's chord) [D].
3. Ease kinds map to `bevy::math::curve::EaseFunction` via a local serde enum
   (§4). Named presets + a **default-ease setting** (fixes "block linear THEN
   ease THEN customize is patently ridiculous") [D].
4. Graph view, when wanted, is a **toggle overlay on the same rows**, value
   graph only, selected curves only by default [D].

---

## 3. The entity browser

The `@pierre/trees` model, translated to bevy_ui. A docked left panel (pop-out
capable) listing spawnable entities + the focused arena's layers. **The palette
is data**: `PaletteRegistry { entries: Vec<PaletteEntry { path, label, icon,
spawn }> }` — paths like `enemies/swarm/wasp` ARE the tree (directories are
derived from segments); amending the toolbar = adding a row (code or a
hot-reloaded `assets/palette/*.ron` manifest).

### 3.1 Virtualization (the trees.software bar)

- Fixed row height (28 px; DM Mono digits), **fixed row pool** =
  `ceil(viewport/ROW_H) + 2×10` overscan rows, pre-spawned once.
- `start = floor(scroll.y / ROW_H)`; if still inside the mounted window, **do
  nothing** (most scroll frames re-mount nothing — translate only). Otherwise
  offset the window container and REBIND pooled rows (write `Text`, swap icon,
  set indent/tints) — never despawn/respawn on scroll.
- A spacer child of `rows.len() × ROW_H` gives the scrollbar honest extent;
  viewport = `Overflow::scroll_y()` + `ScrollPosition`; wheel via an
  `On<Pointer<Scroll>>` observer (bevy_ui has no built-in wheel handling).
- The flat row projection rebuilds ONLY when expansion/filter/registry
  revision change — never on scroll or focus moves.

### 3.2 Keyboard grammar (W3C tree pattern + trees' layers)

`E` focuses the panel (game keeps running). Then: `↓/↑` move · `→` expand or
next · `←` collapse or parent · `Home/End` · `Shift+↓/↑` extend ·
`Ctrl+Space` toggle-select · **any letter opens type-to-filter seeded with
that letter** — normalized substring-on-path match (predictable, not fuzzy),
expand-matches mode; `↓/↑` cycle matches; `Enter` commits and restores the
focused row to its pre-search viewport offset; `Esc` restores the pre-search
expansion snapshot; `Esc` again leaves the panel. Layered Esc: menu → filter →
panel (innermost only).

### 3.3 Actions

- `Enter` on a palette entry = **add as a new layer at the playhead** on the
  focused arena (minion spawn_tick = playhead; tile layer = empty).
- **Drag the selection, not the row** (if the grabbed row is selected, all
  selected entries drag; folder+descendant dedups to the folder). Drop on a
  board tile = spawn at that tick + tile; drop outside = reject with a shake.
- `F2` rename layer rows; `Shift+F10` context menu (add effect, duplicate,
  delete — the right-click "add effects" affordance).

---

## 4. Effect tracks (non-destructive, GPU path)

`EffectTrack { kind: Scale | Opacity, keys: Vec<EffectKey { tick, value,
ease: EaseKind }> }` per layer. `EaseKind` is a **local serde enum** mapped to
`bevy::math::curve::EaseFunction` (bevy's enum is `#[non_exhaustive]` and its
serde sits behind the non-default `serialize` feature; a local enum keeps the
file format stable). Evaluation = `EasingCurve` + `sample_clamped` between
adjacent keys at the arena clock tick; works during playback AND scrubbing.

Application (the CSS no-repaint analogy, verified):

- **Scale** → `Transform.scale` on the layer's root entities. Propagation-only
  cost; no asset churn, no batch break.
- **Opacity** → **clone-once** per-entity `StandardMaterial` instances with
  `AlphaMode::Blend`; per-frame mutation touches only the clone's
  `base_color` alpha (one material uniform re-upload). glTF shells clone their
  descendants' materials on `SceneInstanceReady`. Shared materials are NEVER
  mutated (the storybook and other arenas are unaffected).
- Future zero-clone path: `MeshTag` + a `MaterialExtension` reading
  per-instance params — noted, not v1.

Effects are presentation-layer: exempt from the strict-determinism sim
doctrine (f32 easing is fine — they never feed record/replay state), and they
serialize with the layer so a published stack carries its look.

---

## 5. Panels & pop-out windows

| Capability | Native (macOS/Win/Linux) | Web (wasm) |
|---|---|---|
| Dope sheet / entity browser docked | ✓ | ✓ |
| Pop out into an OS window | ✓ (below) | ✗ compiled out |
| Drag panel between windows | ✓ (re-parent root) | ✗ |

Pop-out (all 0.18-verified): spawn a `Window` entity (it IS the window) + a
`Camera2d { order: 1 }` with `RenderTarget::Window(WindowRef::Entity(win))`
(`RenderTarget` is a separate component in 0.18, not a `Camera` field), and
re-parent the panel's UI root under a root carrying `UiTargetCamera(camera)`.
Per-window `Interaction` + `ui_picking` `Pointer<*>` events are confirmed
working. Window lifecycle events are **Messages** (`WindowClosed` etc., via
`MessageReader`); OS-closing a pop-out re-docks the panel; `DespawnOnExit`
on window+camera+root tears down on state exit. **Panel roots must be
host-agnostic** — built once, re-parented freely (enforced from ARE-41/43 on).

On wasm a second `Window` is just a second canvas that webgl2 can't drive —
the pop-out affordance does not exist there; panels stay docked + collapsible.

---

## 6. Ticket map

| Ticket | Delivers |
|---|---|
| ARE-37 | this document |
| ARE-38 | §1 data model, layered file, fold + conflicts, legacy fallback |
| ARE-39 | author flows on layers (possession roster, draft commit, publish, sync) |
| ARE-40 | §4 effect tracks |
| ARE-41 | §2.1–2.3 dope sheet structure (rows, ruler, scrub, zoom) |
| ARE-42 | §2.4–2.7 keyframe ergonomics + easing panel |
| ARE-43 | §3 entity browser |
| ARE-44 | §1.4 minion layers |
| ARE-45 | §5 pop-out windows |
| ARE-46 | monomorphic aim payloads |
