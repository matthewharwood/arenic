# arenic

A game built in **Rust** (edition 2024) on **Bevy `0.18.1`** (see `Cargo.toml`).

## Workspace layout

A Cargo workspace with one library and two binaries:

- `crates/arenic_game` — **library** of shared, reusable game pieces (components,
  abilities, widgets, engine setup like `default_font`/`ui`). Both binaries depend
  on it; it depends on neither. Anything shared between the game and the storybook
  lives here and is `pub`, never duplicated.
- `crates/arenic` — the **game** binary (`cargo run -p arenic`). Owns its app
  scaffolding (`states`, `title_screen`, `intro_scene`).
- `crates/arenic_storybook` — a **standalone** binary (`cargo run -p arenic_storybook`)
  for building/exercising game pieces in isolation (an IDE-style story tree). It
  imports real components from `arenic_game` to test them outside the game loop.

Notes: `assets/` lives at the workspace root and is shared — `.cargo/config.toml`
pins `BEVY_ASSET_ROOT` there so `cargo run -p <crate>` resolves it correctly.
New shareable game code goes in `arenic_game`; keep the storybook self-contained.

## Bevy 0.18 — hard rule for all Bevy code

**Before writing, editing, or reviewing ANY Bevy (`bevy` / `bevy_*`) code in this repo, consult the [`bevy-018` skill](.claude/skills/bevy-018/SKILL.md) and emit ONLY Bevy 0.18 APIs.** Most Bevy code in training data is pre-0.18 and will not compile here. This applies to every command, agent, and edit.

When you touch Bevy types, components, systems, queries, schedules, assets, or plugins, treat the skill as authoritative: the left side of each `OLD -> NEW` mapping is forbidden; the right side is the only correct form. If a symbol isn't in the skill, verify it against <https://docs.rs/bevy/0.18.1/bevy/> rather than guessing from memory.

The eight non-negotiables (full detail + ~360 mappings in the skill):

1. **No `*Bundle` types** — they're deleted. Spawn components directly; required components fill the rest. `spawn((A, B, C))` — the tuple is the bundle.
2. **No bare `Handle<T>` as a component** — use `Mesh3d`/`Mesh2d`/`MeshMaterial3d`/`MeshMaterial2d`/`SceneRoot`/`AudioPlayer`.
3. **Buffered events are `Message`** (`MessageReader`/`MessageWriter`/`write`/`read`). `#[derive(Event)]` + `On<E>` is observer-only; targeted observer events use `#[derive(EntityEvent)]`.
4. **Observers take `On<E>`, never `Trigger<E>`** — lifecycle events are `Add`/`Insert`/`Replace`/`Remove`/`Despawn`.
5. **`add_systems(Schedule, ..)` always** — explicit schedule, no `.system()`, no `add_system`, no stages.
6. **Colors are sRGB-explicit** — `Color::srgb(..)`, not `Color::rgb(..)`; palettes in `bevy::color::palettes::css`.
7. **`Query::single()`/`single_mut()` return `Result`** — use `?` or the `Single<&T>` param; `get_single` is deprecated.
8. **Hierarchy is `ChildOf` + `Children`** — build with `children![..]`, read parent via `child_of.parent()`; `despawn()` is recursive by default.
