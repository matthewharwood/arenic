# Units & Scale Reference — Camera, Game World, Blender, Window

A single source of truth for how **pixels**, **game world units**, and **Blender
units** relate in Arenic. Use this whenever you author art (Blender → glTF),
place entities, tune the camera, or reason about on-screen sizes.

> **STATUS — read first.** This documents the **target** 3D camera/scale model,
> mirrored from the reference project
> [`arenic_bevy`](https://github.com/matthewharwood/arenic_bevy) (**Bevy 0.16.1**).
> As of **2026-06-01**, *this* `arenic` repo (**Bevy 0.18.1**) is still a 2D
> placeholder — `src/main.rs` spawns only a `Camera2d` under bare
> `DefaultPlugins`; the constants, perspective camera, and window override below
> do **not** exist in this repo's `src/` yet. Treat this as the spec to
> implement. **Every file path captioned `arenic_bevy/...` is in the reference
> repo, not here.** All Rust snippets have been converted to **Bevy 0.18** APIs
> (the reference's 0.16 code differs — see [§8](#8-bevy-016--018-porting-notes)).

---

## TL;DR

| Thing | Value |
|---|---|
| 1 game world unit | 1 meter *by convention* (Bevy doesn't enforce it; glTF mandates meters) = 1 Blender unit |
| Tile size | `0.25` units (a `0.25 × 0.25 × 0.25` cube) |
| Player / character | Sphere **radius `0.125`** → diameter `0.25` = **one tile** |
| Boss | Sphere radius `0.5` (`0.125 × 4`) → diameter `1.0` = 4 tiles across |
| Projectile | Sphere radius `0.0625` → diameter `0.125` = ½ tile |
| Window resolution | `1280 × 720` **physical** px (16:9) |
| Camera | Perspective, **vertical** FOV `π/8` (22.5°), at `z = 24` (in) / `z = 72` (out) |
| **On-screen scale** | **≈ 75.4 px / unit → 1 tile ≈ 19 px** (zoomed in) |

**A "19 × 19 px" player token = one tile = a `0.25`-unit-diameter shape.**
The 19 px is *emergent from the camera*, not an authored constant.

---

## Mental model

The whole game is a **flat board on the XY plane** viewed by a camera parked far
back on **+Z**, looking straight down **−Z**. Because the board is perpendicular
to the view and the lens is narrow (telephoto-like), the perspective camera
behaves almost exactly like an orthographic one: a fixed number of pixels per
world unit at a given zoom.

```
              camera  (eye at z = 24, looks toward −Z)
                 |\
                 | \        vertical FOV = 22.5°
                 |  \
   visible       |   \___  visible_height = 2·d·tan(fov/2)
   height   <----|---/         = 2·24·tan(11.25°) = 9.5478 units
   maps to       |  /          → mapped onto 720 px → 75.4 px/unit
   720 px        | /
                 |/
   ==============+==============   arena on the XY plane
   0            8.125         16.5  (16.5 × 7.75 units, +Z points at camera)
   x →
```

Everything else in this doc is just turning that picture into numbers.

---

## 1. Game world units

Bevy world space is **right-handed, Y-up**: `+X` right, `+Y` up, `+Z` toward the
viewer (so a camera's "forward" is `−Z`). Arenic lays the board flat on the
**XY** plane, which is why `+Z` "points at the camera."

> **"1 unit = 1 meter"** is a *convention* Arenic adopts (so physics, lighting,
> and glTF imports behave naturally), **not** something Bevy enforces — Bevy and
> Godot leave the meaning of a unit to you. glTF *does* mandate meters, which is
> what makes Blender↔Bevy round-trip cleanly (see [§5](#5-blender--game)).

All gameplay distances derive from a single constant
(`arenic_bevy/src/arena/constants.rs`):

```rust
pub const GRID_WIDTH:  u32 = 66;
pub const GRID_HEIGHT: u32 = 31;
pub const TILE_SIZE:   f32 = 0.25;          // units per tile

pub const ARENA_WIDTH:  f32 = GRID_WIDTH  as f32 * TILE_SIZE; // 16.50 units
pub const ARENA_HEIGHT: f32 = GRID_HEIGHT as f32 * TILE_SIZE; //  7.75 units
```

- Tiles are full cubes: `Cuboid::new(TILE_SIZE, TILE_SIZE, TILE_SIZE)`.
- One arena = `66 × 31` tiles = `16.5 × 7.75` units. There are `9` arenas in a
  `3 × 3` layout (`TOTAL_ARENAS = 9`), tiled by `ARENA_WIDTH`/`ARENA_HEIGHT`.
- Grid → world: `get_local_tile_space(col, row, z) = Vec3::new(col * 0.25, row * 0.25, z)`.
  Note `col`/`row` are scaled by `TILE_SIZE` but `z` is **raw units** — callers
  pass the entity's **radius** as `z` so a sphere sits centered one radius above
  the board (e.g. a character at `z = 0.125`, a boss at `z = 0.5`).

### Entity sizes (all spheres in the reference)

| Entity | Radius (units) | Diameter | In tiles | ≈ px @ zoom-in |
|---|---|---|---|---|
| Character / Player | `0.125` (= `TILE_SIZE/2`) | `0.25` | 1 × 1 | ≈ 19 px |
| Boss | `0.5` (= `0.125 × 4`) | `1.0` | 4 across | ≈ 75 px |
| Projectile | `0.0625` (= `TILE_SIZE/4`) | `0.125` | ½ × ½ | ≈ 9.4 px |

> The **player is exactly one tile wide** (radius = ½ tile). Any new player art
> must keep a `0.25`-unit footprint to stay grid-aligned and read as ~19 px.

---

## 2. Window resolution & pixel kinds (logical vs physical)

```rust
// Bevy 0.18 — note u32, not f32 (0.16 took floats):
WindowResolution::new(1280, 720)   // 16:9, PHYSICAL pixels
```

In Bevy 0.18, `WindowResolution::new(physical_width: u32, physical_height: u32)`
sets **physical** pixels — the actual size of the render target / framebuffer the
camera draws into. There are two pixel kinds, and mixing them up is the classic
HiDPI footgun:

| Kind | What it is | In Arenic |
|---|---|---|
| **Physical pixels** | Real pixels in the framebuffer / on the panel | `1280 × 720` (pinned by `new`) |
| **Logical pixels** | DPI-independent "CSS px"; what UI layout uses | `physical / scale_factor` |

The documented relationship is **`physical = logical × scale_factor`**, where
`scale_factor` (a.k.a. device-pixel-ratio) comes from the monitor/OS.

> **Which pixels is "19 px"?** **Physical.** The camera projects into the
> framebuffer, and `new(1280, 720)` pins that framebuffer at `720` physical rows
> *regardless of monitor DPI*. So one tile is ~19 **physical** framebuffer
> pixels on every display. On a Retina/HiDPI Mac (`scale_factor = 2.0`) the
> window merely *appears* physically smaller and its **logical** size is
> `640 × 360` (so the tile is ~9.5 *logical* px), but the render resolution —
> and the 75.4 px/unit math — stays anchored to the 720 physical rows.
>
> ⚠️ This only holds because the resolution is pinned in **physical** pixels. If
> you instead drive the window by a *logical* size or make it freely resizable,
> the framebuffer becomes `logical × scale_factor` and the tile's physical px
> count would scale with DPI. Pin physical px if you want the 19 px to be stable.

If the framebuffer height changes, **px/unit scales linearly with height** (the
vertical FOV is fixed), so all figures below scale by `framebuffer_height / 720`.

---

## 3. Camera

```rust
use std::f32::consts::FRAC_PI_8;

// Bevy 0.18: components, not a *Bundle. Projection is a Component enum.
commands.spawn((
    Camera3d::default(),
    Projection::Perspective(PerspectiveProjection {
        fov: FRAC_PI_8,   // 22.5° — the VERTICAL field of view, in radians
        near: 0.05,
        far:  150.0,
        ..default()       // aspect_ratio is auto-managed — see the gotcha below
    }),
    // eye at (cx, cy, 24), looking straight down −Z onto the board:
    Transform::from_xyz(cx, cy, 24.0).looking_at(Vec3::new(cx, cy, 0.0), Vec3::Y),
));

// Reference (arenic_bevy/src/arena_camera/camera.rs):
pub const ZOOM: (f32, f32) = (24.0, 72.0);  // (zoomed-in z, zoomed-out z)
```

- The camera sits at `(cx, cy, z)` and `look_at`s `(cx, cy, 0)` — eye and target
  share X/Y and differ only in Z, so the view direction is exactly `(0, 0, −1)`:
  **straight down −Z onto the XY board.** Its **z-distance is the "zoom"**: `24`
  framed on one arena, `72` to frame all nine (re-centered on arena index 4).
- It's a true **Perspective** projection, but the board is a flat plane
  perpendicular to the view and the lens is narrow, so it reads as
  near-orthographic (quantified below).

### Two FOV gotchas

1. **`fov` is the *vertical* FOV** (Bevy convention), in radians. Horizontal FOV
   follows from the aspect ratio: `hFOV = 2·atan(tan(vFOV/2) · width/height)`.
2. **The hardcoded `aspect_ratio` is dead config.** Bevy's `camera_system`
   overwrites `PerspectiveProjection.aspect_ratio` every frame with the *actual*
   viewport `width/height`. So the reference's `aspect_ratio: 16.0/9.0` literal
   is cosmetic (it only seeds frame 0). A useful consequence: because Bevy keeps
   the camera aspect equal to the viewport aspect, **px/unit is identical on
   both axes by construction — there's no anamorphic (non-uniform) stretch.**
   Tune `fov`, not the aspect literal.

### Deriving pixels-per-unit (the "19 px" number)

At zoom distance `d`, the **visible vertical extent** on the board is:

```
visible_height = 2 · d · tan(fov / 2)
              = 2 · 24 · tan(11.25°)
              = 9.5478 units            (shown rounded to 9.55 elsewhere)
```

Map onto the 720-physical-px-tall framebuffer:

```
px_per_unit = 720 / 9.5478  ≈ 75.4 px/unit
1 tile      = 0.25 units → 0.25 × 75.41 = 18.85 ≈ 19 px
player      = 0.25 units (1 tile)       ≈ 19 px diameter
boss        = 1.0  units (4 tiles)      ≈ 75 px diameter
projectile  = 0.125 units (½ tile)      ≈ 9.4 px diameter
```

> **Re-derive with the unrounded `9.5478`.** Dividing by the *rounded* `9.55`
> gives `75.39`, not `75.4` — a cosmetic discrepancy, not an error.

**No-distortion proof.** Horizontally, `visible_width = 9.5478 × 16/9 = 16.974`
units → `1280 / 16.974 = 75.41 px/unit`. That's identical to the vertical
`75.41` (difference exactly `0`), so pixels are square and the image is not
stretched — guaranteed by the aspect auto-match above.

| Zoom | Camera z | Visible h (units) | Visible w (units) | px / unit | 1 tile | Frames |
|---|---|---|---|---|---|---|
| In  | `24` | `9.5478`  | `16.974` | `75.41` | **≈ 19 px** | one arena (16.5×7.75 fits, width-bound, ~0.47u margin) |
| Out | `72` | `28.643`  | `50.92`  | `25.14` | ≈ 6.28 px | full 3×3 (49.5×23.25) |

> **Rule of thumb at default zoom: `1 tile ≈ 19 px`, `≈ 75 px per unit`.**

### Why `z = 24` and `z = 72`? (reverse-derivation)

These aren't arbitrary — they're the distances that *frame* the content, with a
small margin. Solve `visible_width = target_width` for `d`:

```
d_fit = target_width / (2 · tan(fov/2) · aspect)

zoom-in : 16.5 / (2·tan(11.25°)·16/9) = 23.33 units → rounded up to 24
zoom-out: 49.5 / (2·tan(11.25°)·16/9) = 69.99 units → 72  (= exactly 3 × 24)
```

- **Width is the binding dimension** (the arena is wider relative to 16:9 than it
  is tall), so framing is set by width; height always has slack.
- `24` rounds `23.33` up to a clean number, leaving a deliberate **~2.9%**
  (0.47-unit total) safety margin so arena edges aren't flush with the screen.
- `72 = 3 × 24` frames the `3 × 3` tiling cleanly and preserves the same ~2.9%
  margin. Change `fov`, `TILE_SIZE`, the grid, or the aspect and these recompute.

### How near-orthographic, exactly?

This perspective-at-distance trick is **perspective / telephoto compression**
("flattening"): a narrow FOV from far away minimizes foreshortening. The 22.5°
vertical FOV is equivalent to a **~60 mm lens** on a full-frame (24 mm-tall)
sensor — a short-telephoto/portrait focal length — i.e. `focal = 12 / tan(11.25°)
≈ 60 mm`. (A 0° FOV would be *exactly* orthographic.)

The residual error is tiny: a character sphere spans `z = −0.125 … +0.125`, so
its near face is `23.875` from the camera and its far face `24.125`. Apparent
size ∝ `1/distance`, so near-vs-far size ratio = `24.125 / 23.875 = 1.0105` —
about **1% across the object's whole depth** (< 0.2 px on a 19 px token).
Conclusion: **treat px/unit as constant at a given zoom.** (If you ever want it
*mathematically* exact, switch to `Projection::Orthographic`.)

---

## 4. Why the precision matters: local space & UI

This is the *reason* `TILE_SIZE` is a clean, uniform `0.25` rather than some
arbitrary value — it has to keep two systems in exact agreement: the
**local-space hierarchy** (how entities are positioned) and the **UI** (how
screen overlays line up with them).

> **Scope check (honesty).** The reference repo (`arenic_bevy`) realizes the
> **local-space** half fully, but has **no screen-space UI at all** — its `ui`
> module is empty; it renders only 3D meshes plus one *world-space* gizmo border.
> The **UI** half (§4.2) is therefore the *integration contract* for when a UI
> layer overlays the grid — which matters because **this** `arenic` workspace
> already ships a UI layer (`crates/arenic_game/src/ui.rs` + the scene crates)
> while the 3D arena/grid isn't ported here yet. §4 is where the two will meet.

### 4.1 Local space — the realized reason for precision

The scene is a three-level **parent/child hierarchy** (`ChildOf` + `Children`):

```
BattleGround (root, Transform default)
└── Arena ×9          Transform = WORLD offset:  (col·16.5, −row·7.75, 0)
    ├── tiles         Transform = LOCAL:         (col·0.25, row·0.25, 0)
    └── characters/   Transform = LOCAL:         get_local_tile_space(col, row, radius)
        bosses
```

Children hold **local** transforms; Bevy's transform propagation computes
`GlobalTransform = arena_world_offset + local`. Placement is always local:

```rust
// arenic_bevy/src/arena/mod.rs
pub fn get_local_tile_space(col: f32, row: f32, z: f32) -> Vec3 {
    Vec3::new(col * TILE_SIZE, row * TILE_SIZE, z) // z is a raw lift (= radius), NOT scaled
}
```

**Why a clean `0.25` is what makes this work:**

- **Drift-free snapping.** `0.25 = 2⁻²` is *exactly* representable in IEEE-754,
  so integer grid coord `N` maps to local `N·0.25` with **zero** rounding error,
  and movement (`translation += dir * TILE_SIZE`, ±0.25/step) never accumulates
  drift — 65 successive `+0.25` steps equal `16.25` *exactly*. A messy size like
  `0.3` would de-center entities over time. Characters stay dead-center on tiles.
- **One layout, nine arenas.** The same constants and the same
  `get_local_tile_space` build every arena's grid and place characters
  identically — because child coords are local, the *same* local position is
  correct in any arena.
- **Edge-to-edge tiling.** Each arena's world offset is an exact integer multiple
  of `ARENA_WIDTH = 16.5` / `ARENA_HEIGHT = 7.75`, so the 3×3 arenas abut with no
  gap or overlap and the camera framing (`col·ARENA_WIDTH`) lands perfectly.
- **Clean seams on reparenting.** Walk off an edge and the character is
  re-`ChildOf`'d to the neighbor arena and teleported to the *opposite* local
  edge (`max_x = (GRID_WIDTH−1)·0.25 = 16.25`, exact) — no sub-tile seam offset.
- **Free zoom / repositioning.** Moving or zooming an arena edits only the Arena
  entity's `Transform`; children never move (the zoom-out in [§3](#3-camera) is
  purely a camera move).

> **Rule:** *write* **`Transform`** (local) to place things on the grid; *read*
> **`GlobalTransform`** when you need true world coordinates (e.g. cross-arena
> distance/targeting, as `auto_shot` does). Confusing the two is the classic
> hierarchy bug.

### 4.2 UI — screen space, and how it aligns to the grid

The crux is a **coordinate-space mismatch** you must respect:

> **Bevy UI is a separate screen-space pass laid out in *logical* pixels — it
> does NOT inherit the 3D camera's projection.** A `Node` sized with `Val::Px`
> is positioned by the flexbox solver in logical px (top-left origin); the
> "≈19 px/tile" world scale from [§3](#3-camera) is **never** applied to UI
> automatically.

Two consequences:

1. **The pixel kinds differ — ties back to [§2](#2-window-resolution--pixel-kinds-logical-vs-physical).**
   The `≈19 px/tile` is **physical** framebuffer px; `Val::Px` is **logical** px.
   On a 2× display the same tile is `≈9.5` *logical* px. Hardcoding either number
   into UI breaks on the other DPI — so **never hardcode "a tile is 19 px"** in a
   `Node`.
2. **Align by projecting, not by constants.** To pin a UI element to a tile or
   entity, project its world position through the camera. Bevy returns the result
   in **logical** px, so it drops straight into `Val::Px` with no scale-factor
   math:

```rust
// Pin a UI Node over a world entity. Bevy 0.18 (fallible system).
fn track_entity_with_ui(
    camera: Single<(&Camera, &GlobalTransform)>,
    target: Single<&GlobalTransform, With<Character>>,
    mut label: Single<&mut Node, With<TrackingLabel>>,
) -> Result {
    let (cam, cam_xf) = *camera;
    let screen = cam.world_to_viewport(cam_xf, target.translation())?; // logical px, top-left
    label.left = Val::Px(screen.x);
    label.top  = Val::Px(screen.y);
    Ok(())
}
```

**Why precise, uniform sizing makes UI tractable:**

- Every tile is identical and the camera scale is fixed, so **every tile projects
  to the same logical-px size at a given zoom** — UI authored in *tile multiples*
  (a 1-tile cursor, a selection ring, a 1-tile-tall health bar) lines up
  consistently across the whole board.
- Positions are **grid-snapped** in local space (§4.1), so their projected screen
  positions are **stable and quantized** — attached UI doesn't jitter sub-pixel.

**Gotchas when projecting:**

- **`UiScale ≠ 1.0`** multiplies `Val::Px` a *second* time (UI-only). If you set
  it, divide the projected value by `UiScale`.
- **Don't feed physical inputs into `Val::Px`** (e.g. raw `physical_viewport_size`
  or physical cursor coords) without dividing by `scale_factor` first.
- `world_to_viewport` returns `Result` — handle off-screen/behind-camera via `?`.

> **Two ways to draw a grid overlay.** A **world-space** overlay (a gizmo or mesh
> — like the reference's `Gizmos.rect` arena border sized `ARENA_WIDTH ×
> ARENA_HEIGHT`) lives in the 3D scene and **inherits the 19 px/tile scale for
> free**. A **screen-space** overlay (`Node`/`Text` HUD) does **not** — it must
> be projected. Use world-space for things glued to the *board* (tile highlights,
> ability ranges); screen-space for things glued to the *screen* (HUD, panels,
> tooltips).

---

## 5. Blender → game

**1 Blender unit = 1 game unit = 1 meter.** Model at true game scale so glTF
imports need no rescaling.

### Why the units round-trip: the glTF standard

The pipeline format is **glTF 2.0** (*GL Transmission Format*, the Khronos open
standard, "the JPEG of 3D"). It is **right-handed, +Y up, with all linear
distances in METERS** — which is exactly why `1 Blender unit = 1 m = 1 game unit`
survives export untouched. `.gltf` is JSON text; **`.glb`** is the single-file
binary container (what we use).

### Axis / orientation gotcha (read this)

Blender is **Z-up**; glTF — and therefore Bevy — is **Y-up**. With the glTF
exporter's default **`+Y Up`** option on, Blender bakes a **Z-up → Y-up axis
conversion** to match the spec:

| In Blender (Z-up) | After glTF import into Bevy (Y-up) |
|---|---|
| Lies flat in XY, faces **+Z** | Lies flat in XZ, faces **+Y** (top-down) |

> Footnote on "forward": glTF defines an asset's front as **+Z**, while Bevy
> defines `Transform::forward()` as **−Z**. The importer reconciles this; you
> rarely touch it, but it's why "forward" looks inverted between the two.

Arenic's board is on the **XY plane with the camera on +Z**, so a flat token
that should *face the camera* needs to end up facing **+Z in Bevy**. Two ways:

1. **Fix at spawn (recommended, non-destructive):**
   ```rust
   Transform::from_rotation(Quat::from_rotation_x(std::f32::consts::FRAC_PI_2))
   ```
2. **Pre-rotate in Blender** so it imports already facing +Z (then apply
   transforms to bake it in).

> This token is a **static flat mesh**, oriented **once** at spawn — it is **not
> a billboard**. Billboarding re-rotates a quad *every frame* to track the
> camera; we don't need or want that here, because the camera looks straight
> down a fixed axis at a perpendicular plane.

### Authoring checklist

- **Scale to the footprint, not to pixels.** A player token is a
  **`0.25`-unit-diameter** shape (radius `0.125`). "19 px" is the *result* of
  that size under the camera in [§3](#3-camera) — never model in pixels. (Unlike
  a 2D engine's per-asset **Pixels-Per-Unit** import setting — Unity defaults to
  100 PPU — Arenic's ~75.4 px/unit is emergent from the camera, not a slider.)
- **Center the origin** at the object's pivot (world origin) for clean transforms.
- **Apply (a.k.a. *freeze*) transforms** — Blender `Ctrl+A → All Transforms`
  (Maya: *Freeze Transformations*) — bakes scale/rotation into the mesh and
  resets the object transform to identity. **Caveat:** do **not** apply
  transforms on armatures/skinned meshes — it can corrupt skin weights. Static
  tokens only.
- **Keep meshes low/clean and texel density modest.** A tile is only ~19 px on
  screen, so high-res geometry/textures are wasted; keep texel density (texels
  per meter) consistent across tokens so the grid reads uniformly.

### Export settings

- Format: **glTF 2.0 Binary (`.glb`)**.
- Path: `assets/models/<name>.glb` (loaded via Bevy `SceneRoot`).
- Include: selected mesh, apply modifiers, **`+Y Up`** (default — leave it on).

---

## 6. Quick conversions & worked example

```
units  → tiles :  units / 0.25
tiles  → units :  tiles * 0.25
units  → pixels:  units * 75.4     (default zoom, 720 physical px tall)
pixels → units :  pixels / 75.4
pixels → tiles :  pixels / 18.85
```

| You want… | Use |
|---|---|
| A 1-tile token (~19 px) | `0.25`-unit diameter shape (radius `0.125`) |
| A 2×2 token (~38 px) | `0.5`-unit diameter (radius `0.25`) |
| Place at grid `(col, row)` | `Vec3::new(col * 0.25, row * 0.25, z)`, `z` = radius |
| Boss-sized (~75 px) | `1.0`-unit diameter (radius `0.5`) |

**Worked example — a 2×2-tile mini-boss token, end to end:**

1. **Size:** 2 tiles = `2 × 0.25 = 0.5` units diameter → radius `0.25`.
2. **On-screen:** `0.5 × 75.4 ≈ 38` physical px at zoom-in.
3. **Blender:** model it `0.5 m` across, origin centered, `Ctrl+A → All
   Transforms`, export `assets/models/miniboss.glb` with `+Y Up`.
4. **Spawn (Bevy 0.18):** at grid `(col, row)` resting on the board —
   ```rust
   commands.spawn((
       SceneRoot(asset_server.load("models/miniboss.glb#Scene0")),
       Transform::from_translation(Vec3::new(col * 0.25, row * 0.25, 0.25)) // z = radius
           .with_rotation(Quat::from_rotation_x(std::f32::consts::FRAC_PI_2)), // face +Z
   ));
   ```

---

## 7. Glossary

- **World unit** — one coordinate unit; `1 unit = 1 m` by convention here.
- **PPU (Pixels Per Unit)** — texture px mapped to one world unit at scale 1 (a
  2D-engine *import* setting; Unity defaults to 100). Arenic's `75.4 px/unit` is
  the *emergent camera analog*, not an import constant — don't look for a slider.
- **Vertical / horizontal FOV** — angular extent the camera sees, per axis.
  Bevy's `fov` is vertical; `hFOV = 2·atan(tan(vFOV/2) · aspect)`.
- **View frustum / near & far clip planes** — the truncated pyramid the camera
  sees; geometry outside `near` (`0.05`) … `far` (`150`) is clipped. The
  projection maps it through **clip space → NDC** (`−1…+1`) → the viewport.
- **Perspective / telephoto compression ("flattening")** — narrow FOV at large
  distance approximating an orthographic look; the basis of the camera here.
- **35 mm-equivalent focal length** — FOV expressed as the lens that gives the
  same view on a full-frame sensor; our 22.5° vFOV ≈ a ~60 mm short-telephoto.
- **Logical vs physical pixels / scale_factor (DPI)** — `physical = logical ×
  scale_factor`. Our px figures are **physical** (pinned by `WindowResolution`).
- **Z-up vs Y-up** — Blender is Z-up; glTF/Bevy are Y-up; the exporter converts.
- **Apply / Freeze transforms** — bake an object's transform into its mesh and
  reset it to identity (skip for skinned meshes).
- **Billboarding** — per-frame rotating a quad to face the camera. We do **not**
  do this; our tokens are statically oriented.
- **SceneRoot** — Bevy 0.18 component that instantiates a loaded glTF scene.

---

## 8. Bevy 0.16 → 0.18 porting notes

The reference (`arenic_bevy`) is **Bevy 0.16**; this repo is **0.18**. When
porting the scale/camera code here, consult the
[`bevy-018` skill](../.claude/skills/bevy-018/SKILL.md) and note at minimum:

- **`WindowResolution::new`** takes **`u32` physical pixels** in 0.18
  (`new(1280, 720)`), not `f32` — the reference's `new(1280.0, 720.0)` won't
  compile here.
- **No `*Bundle` types** — spawn the camera as components
  (`Camera3d`, `Projection::Perspective(..)`, `Transform`); required components
  fill the rest.
- **`PerspectiveProjection.aspect_ratio`** is auto-managed by `camera_system` —
  don't rely on the hardcoded `16.0/9.0`; set the *window* aspect instead.
- Re-verify every other Bevy symbol against the skill before assuming it ports
  unchanged (the pure-Rust constants in [§1](#1-game-world-units) do).

---

## Known discrepancies & caveats (from source audit)

- **Camera vertical centering is off by one tile.** The reference hardcodes the
  per-arena center base as `(8.125, 3.5)`. The X value `8.125` is exactly correct
  (= midpoint of tile-center X `0…16.25`). But the true Y midpoint is **`3.75`**
  (tile-center Y `0…7.5`), so `3.5` sits `0.25` units (one `TILE_SIZE`) **too
  low**. `draw_arena_border` partially compensates with `+TILE_SIZE/2 = 0.125`
  (reaching `3.625`), still `0.125` short. If you port this, prefer `3.75`.
- **Startup arena is `GuildHouse` (index 1)**, so the initial camera is offset
  one `ARENA_WIDTH` in X from the base — not arena 0.
- **Rows grow in −Y**: per-arena `offset_y` is *subtracted*, so the `3×3` grid
  descends in `−Y`.

---

*Everything here flows from four inputs: `TILE_SIZE`, the framebuffer height,
the camera `fov`, and the `ZOOM` distances. If any change, recompute the derived
px/unit numbers — the formulas in [§3](#3-camera) and [§6](#6-quick-conversions--worked-example) show how.*
