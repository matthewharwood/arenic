# Rust: 2024 edition → latest nightly (Bevy-game lens)

What changed in Rust from the **2024 edition (1.85.0, 2025-02-20)** through the
current stable (**1.96.0**, 2026-05-28) and notable nightly features — filtered
for an **Arenic / Bevy 0.18** game on the **nightly** toolchain, **edition 2024**.

**As-of:** 2026-06-02. Local toolchain: `rustc 1.98.0-nightly (6bdf43094 2026-06-01)`.

> **How to read this.** Every feature that earns its place has a **Rule** — a
> blunt, imperative "use it / don't" directive for *this* game. Features that are
> real but irrelevant to gameplay (FFI, inline asm, OS pipes, RISC-V, stdlib
> internals) are listed for completeness **without code examples** and marked
> *Migration-only* or *Not for gameplay*. Code snippets appear **only** where a
> Bevy dev would actually type them.
>
> **Accuracy.** Version→feature mappings are verified against `blog.rust-lang.org`
> / `releases.rs` (see [Sources](#4-sources)); nightly statuses are point-in-time —
> re-check the tracking issue.

---

## Table of contents

1. [The Rust 2024 edition](#1-the-rust-2024-edition)
2. [Per-stable-release highlights after 1.85](#2-per-stable-release-highlights-after-185)
3. [Notable nightly / unstable features](#3-notable-nightly--unstable-features)
4. [Sources](#4-sources)
5. [Summary table](#5-summary-table)

---

## 1. The Rust 2024 edition

Rust **1.85.0** (2025-02-20) shipped the **2024 edition** — opt-in per crate via
`edition = "2024"`. Migrate with:

```bash
cargo build              # green on the current edition first
cargo fix --edition      # auto-apply the `rust-2024-compatibility` rewrites
# then set edition = "2024" in Cargo.toml and re-check
```

> **Rule.** You MUST migrate edition-by-`cargo fix --edition` and never hand-edit
> the mechanical changes below — because the tool inserts the exact `use<>`,
> `unsafe(...)`, and scope fixes for you, and doing them by hand is how you
> introduce the very bugs the edition is trying to prevent.

### 1.1 RPIT (`impl Trait`) captures everything — opt out with `use<>`  **[Bevy]**

In edition 2024, a return-position `impl Trait` captures **all** in-scope generics
**including lifetimes** (matching `async fn`). Narrow it with `use<...>`:

```rust
// captures the &self lifetime by default (2024); add use<> to capture nothing:
fn ids(&self) -> impl Iterator<Item = u32> + use<> { (0..4).map(|i| i) }
```

> **Rule.** When a system helper returns `impl Iterator` / `impl Fn` that does
> **not** actually borrow `&self` or query data, you MUST append `+ use<>` —
> because the 2024 default captures the input lifetime, which silently chains the
> returned iterator to `&self` and makes callers fail to borrow anything else
> until it's dropped. Add only the params you truly borrow (`use<'a, T>`).

**Status.** Default since 1.85 / edition 2024; `use<>` syntax since 1.82 (RFC 3617;
auto-capture RFC 3498). RPIT-in-traits already captured all input lifetimes in every
edition. (Sources: [RFC 3498](https://rust-lang.github.io/rfcs/3498-lifetime-capture-rules-2024.html), [RFC 3617](https://rust-lang.github.io/rfcs/3617-precise-capturing.html), [capture-rules blog](https://blog.rust-lang.org/2024/09/05/impl-trait-capture-rules/).)

### 1.2 `if let` / tail-expression temporary scopes  **[Bevy]**

Edition 2024 drops temporaries **earlier**: those in an `if let` scrutinee die
before the `else` runs, and a block's tail-expression temporaries drop before the
block's locals.

> **Rule.** You MUST NOT assume a lock/guard/resource borrow created inside an
> `if let (...)` scrutinee or a tail expression survives the rest of the block —
> because 2024 drops it sooner. If you need it held, bind it to a `let` first;
> otherwise your "still locked" assumption is now wrong and behavior shifts
> between editions.

**Status.** Edition 2024 (1.85). This earlier-drop rule is what makes **let chains**
(§2, 1.88) sound.

### 1.3 `unsafe_op_in_unsafe_fn` warns by default

Inside an `unsafe fn`, unsafe ops must sit in an explicit `unsafe { }` block.

> **Rule.** In the rare game code that is `unsafe fn`, you MUST wrap each unsafe
> operation in its own `unsafe { }` — because the warning is on by default in
> 2024 and a bare op now fails the `-D warnings` gate. (`cargo fix --edition`
> inserts the blocks.)

**Status.** Warn-by-default, edition 2024 (1.85).

### 1.4 `unsafe extern` blocks · `unsafe` attributes — *Migration-only*

`extern` blocks become `unsafe extern`, and `no_mangle`/`export_name`/`link_section`
become `#[unsafe(...)]`. *Not for gameplay* — a Bevy game almost never declares FFI
or hand-exports symbols (Trunk/Bevy own the wasm entry point). `cargo fix --edition`
rewrites any that exist. No example.

### 1.5 `.await` via `IntoFuture` · never-type fallback · `gen` reserved · macro fragments — *Migration trivia*

All default in 1.85 / edition 2024; `cargo fix` handles them. The only one to *know*:
**`gen` is now a reserved keyword** (an identifier `gen` becomes `r#gen`), reserved
for the `gen` blocks in §3.1. The rest (`.await` accepting `IntoFuture`, never-type
`!` fallback shifts, `expr` matching `const {}`/`_`) you will not hand-write.

### 1.6 Cargo resolver v3 / MSRV-aware  **[Bevy]**

`edition = "2024"` implies `resolver = "3"` ⇒ MSRV-aware dependency resolution
(won't pick a dep version whose `rust-version` exceeds yours).

```toml
# A VIRTUAL workspace root has no [package], so it does NOT inherit the default:
[workspace]
resolver = "3"   # set it explicitly
```

> **Rule.** In this virtual workspace you MUST set `resolver` explicitly in the
> root `Cargo.toml` — because the edition-2024 default only applies to a crate
> with a `[package]`, so a workspace root silently falls back to the ancient
> resolver v1 unification otherwise. (This repo pins it.)

**Status.** Opt-in since 1.84.0; default via edition 2024 in 1.85.0. (Source: [Edition Guide — Cargo resolver](https://doc.rust-lang.org/edition-guide/rust-2024/cargo-resolver.html).)

---

## 2. Per-stable-release highlights after 1.85

> Verified against the official announcements (1.85–1.91, 1.95, 1.96) and
> `releases.rs` (1.92–1.94), accessed 2026-06-02. Each release lists a
> representative slice, not the full changelog.

### Rust 1.85.0 — 2025-02-20  *(also ships edition 2024; see §1)*

- **Async closures (`async || {}`)** + `AsyncFn`/`AsyncFnMut`/`AsyncFnOnce` (RFC 3668).

> **Rule.** Use `async ||` **only** inside genuine async work (asset loaders,
> `AsyncComputeTaskPool` tasks); you MUST NOT push `async` into ECS systems —
> Bevy schedules systems synchronously, so async there adds a runtime and buys
> nothing.

### Rust 1.86.0 — 2025-04-03

- **`get_disjoint_mut`** *(std)* — `&mut` to several slice/`HashMap` elements at once
  (renamed from `get_many_mut`).

  ```rust
  let mut tiles = [0u8; 64];
  if let Ok([a, b]) = tiles.get_disjoint_mut([from, to]) {
      std::mem::swap(a, b); // move a unit between two tiles by index
  }
  ```

  > **Rule.** When you need `&mut` to two-plus elements of the same `Vec`/slice/
  > `HashMap` by index (swapping tiles, updating paired entities), you MUST use
  > `get_disjoint_mut` — because the borrow checker forbids overlapping `&mut`
  > from repeated indexing, and the old `split_at_mut`/`unsafe` workarounds are
  > bug farms.

- **Trait upcasting** — coerce `Box<dyn Sub>` → `Box<dyn Super>`.

  > **Rule.** When you hold `dyn Ability`/`dyn Effect` trait objects with a
  > supertrait, you MUST upcast directly (`let s: &dyn Super = sub;`) instead of
  > adding an `as_super(&self)` shim method — the shim is now dead boilerplate.

### Rust 1.87.0 — 2025-05-15  *(10-year anniversary)*

- **`Vec::extract_if` / `LinkedList::extract_if`** *(std)* — drain matching elements.

  ```rust
  let mut effects = vec![/* … */];
  let expired: Vec<_> = effects.extract_if(.., |e| e.ttl == 0).collect();
  ```

  > **Rule.** To remove-and-collect the elements of a `Vec` that match a predicate
  > (expired effects, dead entities in a side list), you MUST use `extract_if` —
  > because `retain` throws the removed items away and a manual index loop with
  > `swap_remove` reorders and is off-by-one-prone.

- `usize::is_multiple_of` is handy for grid math (`col.is_multiple_of(STRIDE)`).
- **Precise capturing `use<>` in traits** — folds into the §1.1 rule.
- *Not for gameplay:* `asm!` label operands; `std::io::pipe()`. No examples.

### Rust 1.88.0 — 2025-06-26

- **Let chains** *(edition 2024 only)* — `&&`-chain `let` in `if`/`while`.

  ```rust
  if let Some(p) = player && let Some(t) = p.target && t.alive {
      attack(p, t);
  }
  ```

  > **Rule.** You MUST flatten nested `if let { if let { if cond } }` into a single
  > `if let … && let … && cond` — because the workspace `-D warnings` gate fires
  > `clippy::collapsible_if` on the nesting, and chaining is the only readable way
  > to gate a system on several `Option`/component lookups at once.

- `<[T]>::as_chunks` / `as_rchunks` — fixed-size windows over a slice (e.g. vertex
  buffers, RGBA pixels).

  > **Rule.** When you process a slice in fixed-size groups (4 bytes/pixel, N verts
  > per quad), you MUST use `as_chunks::<N>()` over manual `chunks_exact` + unwrap —
  > it yields `&[[T; N]]`, so the group size is in the type and the remainder is
  > handled for you.

- *Not for gameplay:* `#[unsafe(naked)]` functions; `core::ffi::c_str`.
- **Cargo:** auto cache GC (prunes old cached crates). **[Bevy]** keeps the big Bevy
  dep cache from ballooning — nothing to do, just know it happens.

### Rust 1.89.0 — 2025-08-07

- **Inferred (`_`) const-generic args** *(in fn bodies)*.

  ```rust
  fn blank_row() -> [Tile; GRID_W] {
      [Tile::EMPTY; _] // length inferred from the return type
  }
  ```

  > **Rule.** Inside a body, when the array length is already fixed by a `const` or
  > the return type, you MUST write `[x; _]` instead of repeating the count —
  > because hard-coding the number twice drifts the day you resize the grid.
  > (Not allowed in signatures.)

### Rust 1.90.0 — 2025-09-18

- **LLD is the default linker for `x86_64-unknown-linux-gnu`** *(tooling)*.

  > **Rule.** On Linux you MUST stay on the default LLD linker (don't pass
  > `-C linker-features=-lld`) — because Bevy's link step dominates incremental
  > rebuild time, and LLD is dramatically faster on large/debug binaries.

- *Not for gameplay:* `x86_64-apple-darwin` → Tier 2; `sub_signed` integer methods.

### Rust 1.91.0 — 2025-10-30

- **Integer strict arithmetic** (`strict_add`/`strict_mul`/…) — panic on overflow in
  **every** profile.

  > **Rule.** For gameplay math that feeds the deterministic record/replay
  > (tick counters, grid indices, resource totals), you MUST pick an explicit
  > overflow policy — `strict_*` to fail loud, or `wrapping_*` to wrap
  > deterministically — and you MUST NOT use bare `+`/`*`, because it panics in
  > debug but silently wraps in release, so a replay can diverge between builds.

- `Duration::from_mins` / `from_hours` — clearer than `from_secs(n * 60)` for cycle timers.
- *Not for gameplay:* `AtomicPtr` arithmetic; `Ipv*Addr::from_octets`; `Path::file_prefix`.

### Rust 1.92.0 — 2025-12-11

- `Box`/`Rc`/`Arc::new_zeroed` (+ `_slice`) — allocate a zeroed buffer without
  initializing twice.

  > **Rule.** When you allocate a large zero-filled buffer (audio/scratch/image
  > buffers), use `Box::new_zeroed_slice(n)` over `vec![0; n].into_boxed_slice()` —
  > it skips the redundant write the optimizer can't always elide.

- *Niche:* `RwLockWriteGuard::downgrade`; `NonZero::div_ceil`; const `slice::rotate_*`.

### Rust 1.93.0 — 2026-01-22

- **Cargo `clean --workspace`** *(tooling)* — clean every member's artifacts.
- *Niche / not for gameplay:* C-variadics for `system` ABI; `asm_cfg`;
  `Vec::into_raw_parts`; `<[T]>::as_array`; `VecDeque::pop_front_if`.

### Rust 1.94.0 — 2026-03-05

- **`LazyCell` / `LazyLock` accessors** (`get`, `get_mut`, `force_mut`).

  > **Rule.** For a lazily-computed global table (lookup/config built once), you
  > MUST use `LazyLock` — because hand-rolled `OnceLock` + init checks are exactly
  > the boilerplate it removes, and it's `Sync` for use across Bevy's threads.

- *Not for gameplay:* RISC-V target features; `array_windows`/`element_offset`;
  float `EULER_GAMMA`/`GOLDEN_RATIO` constants. **Cargo:** config `include`, TOML v1.1.

### Rust 1.95.0 — 2026-04-16

- **`if let` guards in `match`** — `match x { v if let Some(y) = f(v) => … }`.

  > **Rule.** When a `match` arm needs a *fallible* secondary lookup, you MUST use
  > an `if let` guard instead of matching then nesting an `if let` in the body —
  > it keeps the arm's intent on one line and avoids a catch-all fallthrough arm.

- **`core::hint::cold_path()`** — mark an unlikely branch.

  > **Rule.** In a hot per-entity/per-tick system loop, you MUST call
  > `core::hint::cold_path()` at the top of the rare branch (error/spawn/despawn
  > path) — it biases codegen to keep the common path straight-line and hot.

- **`cfg_select!`** — choose an expr/item by `cfg` at compile time (cleaner than
  stacked `#[cfg]`). Use for per-platform backends.
- *Niche:* `Vec::push_mut`; `MaybeUninit`/`Cell` array conversions; atomic `update`.

### Rust 1.96.0 — 2026-05-28  *(current stable)*

- **`core::range` types** — `Range`/`RangeFrom`/`RangeInclusive` that are **`Copy`**
  (legacy `std::ops::Range` is not).

  ```rust
  use core::range::Range;
  #[derive(Clone, Copy)] struct Band { rows: Range<u32> } // Copy now works
  ```

  > **Rule.** When a range is stored in a `Component`/`Resource` or returned by
  > value, you MUST use `core::range::Range` instead of `a..b` — because the legacy
  > range isn't `Copy`, so it blocks `#[derive(Copy)]` on the component and moves
  > out from under you.

- **`assert_matches!`** — assert a value matches a pattern (great in tests).

  > **Rule.** In tests, you MUST use `assert_matches!(got, Expected::Variant { .. })`
  > over `assert!(matches!(...))` — because it prints the actual value on failure
  > instead of just `false`.

- *Not for gameplay:* wasm undefined-symbol errors (relevant only to the Trunk web
  build config — leave defaults).

---

## 3. Notable nightly / unstable features

> **UNSTABLE** — requires nightly (you're on it) + the named `#![feature(...)]`.
> Status moves fast; re-check the tracking issue.

### 3.1 `gen` blocks & `gen fn` — ergonomic iterators  **[Bevy]**

```rust
#![feature(gen_blocks)]
fn ring(center: IVec2, r: i32) -> impl Iterator<Item = IVec2> {
    gen move { for dx in -r..=r { for dy in -r..=r {
        if dx*dx + dy*dy <= r*r { yield center + IVec2::new(dx, dy); }
    }}}
}
```

> **Rule.** When a system needs to *yield a sequence lazily* (procedural
> generation, spatial/AoE scans, streaming spawns), you MUST reach for a `gen {}`
> block before hand-writing an `Iterator` impl or `std::iter::from_fn` — because
> the manual state machine is pure boilerplate and the usual source of
> off-by-one/early-exit bugs.

**Status.** UNSTABLE — `feature(gen_blocks)`, [#117078](https://github.com/rust-lang/rust/issues/117078). (`async gen` → `Stream` is the same effort.)

### 3.2 Coroutines / generators — *Not for gameplay (substrate)*

The low-level `yield`-in-coroutine primitive `gen` is built on.

> **Rule.** You MUST NOT hand-write raw coroutines — use `gen` blocks (§3.1).
> Coroutines are the compiler substrate, not a user-facing API.

**Status.** UNSTABLE — `feature(coroutines)`, [#43122](https://github.com/rust-lang/rust/issues/43122).

### 3.3 Return-type notation (RTN)

Bound the future of an `async fn` in a trait, e.g. `where S::fetch(..): Send`.

> **Rule.** Only if you define your **own** `async fn` in a trait **and** need its
> future to be `Send` for Bevy's multithreaded scheduler do you reach for RTN —
> otherwise you MUST avoid `async fn` in your traits entirely (keep async at the
> task/`Future` boundary), because the `Send`-bound gap is the whole reason this
> is still painful.

**Status.** UNSTABLE — `feature(return_type_notation)` (RFC 3654, [#109417](https://github.com/rust-lang/rust/issues/109417)); stabilization PR [#138424](https://github.com/rust-lang/rust/pull/138424) closed unmerged 2025-12-27, so nightly-only as of mid-2026. (Base `async fn` in traits is **stable since 1.75**.)

### 3.4 `generic_const_exprs` — *Avoid in shipping code*

Const arithmetic in generics (`[T; N + 1]`). Tracking [#76560](https://github.com/rust-lang/rust/issues/76560).

> **Rule.** You MUST NOT use `generic_const_exprs` in Arenic — it is explicitly
> **incomplete and unsound**, so it can ICE or silently miscompile. For fixed-size
> game math, pin a plain `const` or accept a small runtime `Vec`/`SmallVec`.

**Status.** UNSTABLE & incomplete — `feature(generic_const_exprs)` + `allow(incomplete_features)`.

### 3.5 Specialization — *Do not use*

Overriding a `default` impl with a specific one. Tracking [#31844](https://github.com/rust-lang/rust/issues/31844).

> **Rule.** You MUST NOT use `specialization` — it has known soundness holes and is
> not near stabilization. If you think you need it, redesign with an enum or a
> trait method instead.

**Status.** UNSTABLE / blocked on soundness (the sound subset `min_specialization` is stdlib-internal).

---

## 4. Sources

Verified 2026-06-02.

**Release announcements (blog.rust-lang.org):**
[1.85.0](https://blog.rust-lang.org/2025/02/20/Rust-1.85.0/) ·
[1.86.0](https://blog.rust-lang.org/2025/04/03/Rust-1.86.0/) ·
[1.87.0](https://blog.rust-lang.org/2025/05/15/Rust-1.87.0/) ·
[1.88.0](https://blog.rust-lang.org/2025/06/26/Rust-1.88.0/) ·
[1.89.0](https://blog.rust-lang.org/2025/08/07/Rust-1.89.0/) ·
[1.90.0](https://blog.rust-lang.org/2025/09/18/Rust-1.90.0/) ·
[1.91.0](https://blog.rust-lang.org/2025/10/30/Rust-1.91.0/) ·
[1.95.0](https://blog.rust-lang.org/2026/04/16/Rust-1.95.0/) ·
[1.96.0](https://blog.rust-lang.org/2026/05/28/Rust-1.96.0/) ·
[impl Trait capture rules](https://blog.rust-lang.org/2024/09/05/impl-trait-capture-rules/) ·
[async fn / RPIT in traits](https://blog.rust-lang.org/2023/12/21/async-fn-rpit-in-traits/)

**Changelogs (releases.rs):**
[1.92.0](https://releases.rs/docs/1.92.0/) · [1.93.0](https://releases.rs/docs/1.93.0/) · [1.94.0](https://releases.rs/docs/1.94.0/)

**Edition Guide / RFCs:**
[Rust 2024](https://doc.rust-lang.org/edition-guide/rust-2024/) ·
[Cargo resolver](https://doc.rust-lang.org/edition-guide/rust-2024/cargo-resolver.html) ·
[RFC 3498](https://rust-lang.github.io/rfcs/3498-lifetime-capture-rules-2024.html) ·
[RFC 3617](https://rust-lang.github.io/rfcs/3617-precise-capturing.html) ·
[RFC 3654](https://rust-lang.github.io/rfcs/3654-return-type-notation.html)

**Tracking issues:**
`gen_blocks` [#117078](https://github.com/rust-lang/rust/issues/117078) ·
coroutines [#43122](https://github.com/rust-lang/rust/issues/43122) ·
RTN [#109417](https://github.com/rust-lang/rust/issues/109417) ·
`generic_const_exprs` [#76560](https://github.com/rust-lang/rust/issues/76560) ·
specialization [#31844](https://github.com/rust-lang/rust/issues/31844)

> Built via a multi-agent deep-research pass (parallel search → primary-source
> fetch → adversarial version verification), then cross-checked release-by-release
> against the official notes. Nightly statuses are point-in-time (2026-06-02).

---

## 5. Summary table

Game relevance: ✅ use it · ⚠️ situational · ⛔ avoid / migration-only.

| Feature | Version / status | Rule (short) |
|---|---|---|
| RPIT capture + `use<>` | 1.85 / edition 2024 | ✅ add `+ use<>` when the return doesn't borrow inputs |
| `if let` / tail-expr scope | 1.85 / edition 2024 | ⚠️ don't assume guards live to end-of-block; bind them |
| `unsafe extern` / `unsafe` attrs | 1.85 / edition 2024 | ⛔ migration-only; `cargo fix` handles it |
| Cargo resolver v3 (MSRV-aware) | 1.84 opt-in · 1.85 default | ✅ set `resolver` explicitly in the virtual workspace root |
| Async closures `AsyncFn*` | 1.85 | ⚠️ only in async tasks, never in ECS systems |
| `get_disjoint_mut` | 1.86 | ✅ for `&mut` to multiple slice/map elements by index |
| Trait upcasting | 1.86 | ✅ upcast `dyn Sub`→`dyn Super` instead of shim methods |
| `Vec::extract_if` | 1.87 | ✅ to remove-and-collect matching elements |
| `as_chunks::<N>` | 1.88 | ✅ for fixed-size slice groups (pixels/verts) |
| **Let chains** | 1.88 / edition 2024 | ✅ flatten nested `if let`; the gate requires it |
| Naked fns / `asm!` / `io::pipe` | 1.87–1.88 | ⛔ not for gameplay |
| Inferred `_` const-generic args | 1.89 | ✅ `[x; _]` in bodies; don't repeat the length |
| **LLD default linker (linux)** | 1.90 | ✅ keep it on — Bevy link speed |
| Strict integer arithmetic | 1.91 | ✅ explicit `strict_*`/`wrapping_*` for replay-deterministic math |
| `*::new_zeroed[_slice]` | 1.92 | ⚠️ for large zeroed buffers |
| `cargo clean --workspace` | 1.93 | ✅ tooling |
| `LazyLock`/`LazyCell` accessors | 1.94 | ✅ for compute-once global tables |
| `if let` match guards · `cold_path` | 1.95 | ✅ fallible guards; mark cold branches in hot loops |
| `cfg_select!` | 1.95 | ⚠️ per-platform backend selection |
| **`core::range` (Copy ranges)** | 1.96 (latest) | ✅ store ranges in components/resources |
| `assert_matches!` | 1.96 | ✅ in tests |
| `gen` blocks | UNSTABLE #117078 | ✅ lazy sequences (procgen/scans) over manual `Iterator` |
| Coroutines | UNSTABLE #43122 | ⛔ substrate — use `gen` |
| Return-type notation | UNSTABLE #109417 | ⚠️ only for `Send` bounds on your async-trait futures |
| `generic_const_exprs` | UNSTABLE/incomplete #76560 | ⛔ unsound — do not ship |
| Specialization | UNSTABLE/blocked #31844 | ⛔ do not use |
