---
description: Sweep the workspace refactoring Bevy code to the 0.18 skill + code-review policy
argument-hint: "[path | -p <crate> | --changed]  (default: all three game crates)"
---

# /refactor-bevy

Refactor Rust/Bevy code across this workspace so it conforms to **both** authoritative
rule sources, then prove it still compiles. This is a *conformance* refactor — it
modernizes APIs and applies house policy. It must **not** redesign gameplay.

## Authorities (read both before touching code)

1. **Mechanics — `.claude/skills/bevy-018/SKILL.md`.** Every OLD → NEW API mapping.
   The left side of any mapping is forbidden; the right side is the only correct
   form. If a symbol isn't in the skill, verify it against
   <https://docs.rs/bevy/0.18.1/bevy/> — never from memory.
2. **Judgment — `_docs/code-review.md`.** Architecture, determinism, naming, and the
   Rust-2024/nightly idioms. This is *how to choose* among valid 0.18 forms.

Also honor `CLAUDE.md`: front-door imports (re-export from `mod.rs`/`lib.rs`, import
the short path), files under ~1000 lines (split on a real fault line, don't append),
DM Sans / DM Mono font rules, theme tokens over ad-hoc colours, no `.sh` scripts
(automation goes in `xtask`).

## Scope (`$ARGUMENTS`)

- *empty* → the three game crates: `crates/arenic/src`, `crates/arenic_game/src`,
  `crates/arenic_storybook/src`.
- a path or glob → just those files/dirs.
- `-p <crate>` → that crate only.
- `--changed` → files in `git diff --name-only` (and staged) ending in `.rs`.

`xtask` is excluded from the clippy gate but its Bevy-adjacent code (if any) is still
migratable; only touch it when explicitly in scope.

## Procedure

1. **Baseline.** Run `cargo build` (or `-p <crate>` for narrow scope) and confirm it
   is green *before* editing. Note the existing `cargo clippy --workspace` warning
   count so you can prove you didn't regress it.
2. **Enumerate** the target `.rs` files from the scope rules above.
3. **Refactor in ordered passes, safest first.** Per file (or per crate):

   - **Pass 1 — mechanical (skill).** Replace every pre-0.18 API with its 0.18 form:
     `*Bundle` → required components; bare `Handle<T>` → `Mesh3d`/`MeshMaterial3d`/
     `SceneRoot`/`AudioPlayer`; `EventReader`/`EventWriter`/`.send` → `Message*`/
     `.write`; `Trigger<E>` → `On<E>`; `Color::rgb` → `Color::srgb`;
     `Input<KeyCode>` → `ButtonInput`; `KeyCode::W/Up/Key1` → `KeyW/ArrowUp/Digit1`;
     `add_system*` → `add_systems(Schedule, …)`; `Parent`/`with_children` →
     `ChildOf`/`children![..]`; `despawn_recursive` → `despawn`; `get_single` →
     `single()?`; `time.delta_seconds()` → `delta_secs()`; `shape::*` → `bevy_math`
     primitives; `.label`/`SystemLabel` → `.in_set`/`SystemSet`. Apply every
     applicable skill mapping, not just these.

   - **Pass 2 — policy (code-review.md).** Rip out forwarding getters/setters in
     favour of `pub` fields; drop `Component`/`Tag`/`Data` type-name suffixes; move
     per-entity state out of Resources onto entities; replace bare `.unwrap()` in
     systems with `?` or `.expect("invariant: …")`; replace bare `+`/`-`/`*` in
     tick/grid/replay math with `strict_*`/`wrapping_*`; convert set-once components
     to `#[component(immutable)]`; turn one-shot polling systems into observers
     where it fits; ensure recorded data stores intent, not derived results.

   - **Pass 3 — idioms (Rust 2024 / nightly).** Collapse nested `if let` into let
     chains; `get_disjoint_mut` for paired index mutation; `extract_if` for
     remove-and-collect; `LazyLock` for compute-once globals; `core::range::Range`
     for stored ranges; `[x; _]` for inferred array lengths; `as_chunks::<N>` for
     fixed groups; `gen {}` for lazy sequences (only if `gen_blocks` is already
     enabled); `cold_path()` in hot rare branches; `assert_matches!` in tests.

4. **Preserve behavior.** This refactor changes *form*, not gameplay. When a change
   is semantically risky — making a hot component immutable, converting direct
   mutation to an event, reordering systems — **don't guess. Flag it** and ask, or
   leave it and list it under "needs a human call."
5. **Verify per crate** as you finish it: `cargo build -p <crate>` then
   `cargo clippy -p <crate>` (workspace lints, `-D warnings`). Fix the fallout before
   moving on. Run any tests the crate has.
6. **Don't commit** unless asked. Leave the working tree changed and summarize.

## Output

- A per-pass changelog grouped by file (what changed and why).
- A **"needs a human call"** list of the risky/ambiguous items you deliberately
  skipped, each with the trade-off.
- The final `cargo build` + `cargo clippy` result, compared to the baseline.

## Scale

For a wide scope this is a lot of mechanical edits across many files. Two paths:

- **Inline (default).** Walk crate-by-crate; verify each before the next so a break
  is localized.
- **Workflow (large sweeps / `ultracode`).** Pipeline the file list: per file,
  *transform → build-verify*, with `isolation: 'worktree'` when transforming files
  in parallel so concurrent edits don't collide; barrier-merge per crate, then run
  `cargo clippy -p <crate>` once. Adversarially re-check any Pass-2 semantic change
  (immutable conversion, event vs direct mutation) with a second agent before
  keeping it.

After the refactor, run `/code-review` for a correctness pass over the diff.
