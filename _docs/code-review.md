Your Software Engineering Values
- Clarity: Self-evident code that junior engineers can understand.
- Simplicity: The minimum complexity required, no more.
- Conciseness: Every line serves a purpose.
- Elegance: Beautiful solutions to complex problems.
- Self-documenting: Code explains itself through naming and structure.
- Consistency: Patterns that scale across the codebase.
- Efficiency: Optimal algorithms and data structures.
- Performance: Frame time is sacred—profile first, measure, then optimize with statistical validation.
- Scalability: Systems that handle 10 or 10,000 entities.
- Predictable & Deterministic: Frame-rate independent, idempotent operations with explicit coordination and bounded concurrency.
- Modularity: Plugins/systems that compose cleanly.
- Extensibility: Today's code supports tomorrow's features.
- Flexibility: Static data for designers, dynamic systems for players.
- Testability: Every system provable in isolation.
- Cohesion/Decoupling: Related code lives together; systems communicate via events.
- Usability: APIs that are hard to misuse.
- Configurability where it matters.

# Bevy 0.18 code-review rules

These supersede the older "29 Rules / Type Domain absolute" checklist, which was
written for a pre-observer Bevy (0.15/0.16) and flags correct 0.18 code as
violations.

**Two layers, no overlap.** The **`bevy-018` skill** is the *mechanics* layer —
every OLD → NEW API mapping (no `*Bundle`; `Message` vs `Event`; `On<E>` observers;
`ChildOf`; `single() -> Result`; `Has`/`iter_many`/`par_iter`; `set_if_neq`; srgb
colors; `ButtonInput`; `Time<Fixed>`; …). This file is the *judgment* layer: what to
build, why, and which of several valid 0.18 forms to choose. It deliberately does
**not** restate skill mappings — if a rule here would just repeat an OLD→NEW line,
it has been cut. Consult the skill for the API; consult this for the call. When a
symbol isn't in the skill, verify against <https://docs.rs/bevy/0.18.1/bevy/>, never
from memory.

## Data flow

Bevy data flow is **sources converging into one World, then a per-frame loop that
reads and mutates it** — not a linear conveyor belt. Two corrections to the common
"const → assets → components → events → render" sketch: (1) `const` tables and
loaded assets are **parallel input sources**, not sequential — a `const` never
"becomes" a `Handle<T>`; (2) most mutation is **direct `Query<&mut T>`**, not
events, and change detection is a *reaction* optimization, not the render path.

```
SOURCES OF TRUTH (converge into the World)
├─ const tables ............ designer/stat data baked in the binary
├─ Assets via AssetServer .. Handle<T> held in wrappers (Mesh3d, SceneRoot…)
├─ Resources ............... global config & singletons
└─ Input ................... ButtonInput, window/gamepad as Messages
        │
        ▼
SETUP  (Startup)
  spawn entities → Components (Required Components auto-fill the rest)
  insert Resources;  init_state for the app FSM
        │
        ▼
╔═════════════ PER-FRAME LOOP (Update / FixedUpdate) ═════════════╗
║  READ      Queries  +  Res<T>  +  MessageReader                 ║
║    │                                                            ║
║  DECIDE /  buffered streams → Message ;  reactive/targeted →    ║
║  COMMUNICATE   Event + On<E> observers                          ║
║    │                                                            ║
║  MUTATE    Query<&mut T>  (the happy path)                      ║
║            structural add/remove/spawn → Commands               ║
║    │                                                            ║
║  REACT     Changed<T> / Added<T> , RemovedComponents::read() ,  ║
║            lifecycle observers (Add/Insert/Remove) , hooks      ║
╚════════════════════════════════════════════════════════════════╝
        │
        ▼
RENDER (extract reads Transform, Mesh3d, MeshMaterial3d, … each frame)
```

Review takeaways from the diagram:

- `const` data and assets are **siblings**, not a chain. Don't model designer
  tables as something that "loads into" a handle.
- **Mutate with `Query<&mut T>` first.** Route through `Message`/`Event` only when
  the decoupling earns its keep; structural changes go through `Commands`.
- **Change detection is a REACT-stage tool**, letting systems skip unchanged work —
  it is *not* "how rendering happens." The render extract reads components every
  frame regardless.
- Resources, States, and `Commands` are first-class parts of the flow, not
  afterthoughts.

## Hard requirements (real gates)

The mechanical 0.18 gates — no `*Bundle`, `Handle<T>` wrappers, `Message` vs
`Event`, `On<E>` observers, srgb colors, `single() -> Result` — are the
**`bevy-018` skill**'s job; treat every OLD form there as a STOP. The gates below
are policy the skill does not encode:

- **Errors propagate, they don't panic.** Systems are fallible — return `Result`,
  use `?`, domain errors via `thiserror`. A bare `.unwrap()` in a system → STOP.
  Use `.expect("invariant: …")` *only* to assert a genuine invariant (clearer than
  a silent early-return); tests / `xtask` / setup code are exempt. Never silently
  drop a `Result` — add context where the origin would be unclear. Configure one
  error handler in `main`: panic in dev (default), log in production.
- **Deterministic sim math → no bare arithmetic.** In any code feeding record/
  replay — tick counters, grid indices, RNG, resource totals — never use bare
  `+`/`-`/`*` (panics in debug, *wraps* in release, so a replay diverges between
  builds). Pick an explicit policy per call: `strict_*` to fail loud, or
  `wrapping_*` to wrap deterministically. See **Determinism** below.

## Type domains — corrected

The old "a Component can NEVER be an event payload" rule is **false in 0.18**:
`#[derive(Event)]` *implies* `Component`, so every observer event *is* a
component. Apply the intent, not the absolutism:

- Model stored entity state and transient messages as separate types **when their
  data or lifecycle differ**; reference other entities by `Entity`, never by their
  components. Don't force a second type when the same plain struct serves both as
  state and payload — observer events being components is by design.
- For **`Message`** payloads (buffered streams), keep them plain data and point at
  entities with `Entity` — don't smuggle stored-state components through the buffer.
- For **`Event`/`EntityEvent`** payloads (observers), the type being a component is
  correct; do not "fix" it. `EntityEvent` *requires* an `entity: Entity` field.
- Passing a component to a **helper function** (`fn apply(h: &mut Health, ..)`) is
  fine and idiomatic. "No Component as a system parameter outside a Query" is
  already enforced by the compiler — a bare component isn't a valid `SystemParam`.
- Cross-boundary communication should flow through `Message`/`Event` where the
  decoupling earns its keep — but see "Direct mutation is the happy path"; not
  *every* change needs an event.

## ECS modeling

- **Marker components.** Zero-sized markers for categorization/toggles —
  `struct Boss;`, never `struct Boss(bool)`. A toggle is presence/absence of the
  marker (or an enum), not a `bool` field.
- **Component composition.** Many small, single-purpose components compose into
  complex behavior. Components are not general-purpose data bags.
- **Entity state on entities, not in Resources.** Reach for a Resource only for
  genuine global singletons (config, `Time`, `AssetServer` handles, a shared
  registry). If a piece of data belongs to "a thing in the world," it's a
  Component — don't park per-entity (or per-few-entities) state in a Resource.
- **`Commands` for structural change; direct `&mut World` only in exclusive
  systems** — and only when you genuinely need whole-World access, because it
  serializes the schedule.
- **Process only what changed.** Drive reactive work off `Added<T>`/`Changed<T>`
  (removals via `RemovedComponents`); don't re-scan every entity each frame.
- **Observers over polling for one-shot reactions.** If a system exists only to
  notice "X happened and respond," prefer an observer (`On<Add, T>`, `On<MyEvent>`)
  over a per-frame query that re-checks. Keep polling systems for continuous/
  aggregate work; use observers for discrete, targeted reactions.
- **Query for batches, not random lookups.** Filter to the set you want and
  iterate it; reach across entities with the batched accessors rather than a loop
  of single `get()`s; cache reused handles locally; parallelize when iteration
  order is irrelevant.
- **Parallelism is the default — don't break it needlessly.** Disjoint systems run
  in parallel automatically; avoid exclusive systems and broad `ResMut` unless
  required. Never *assume* execution order — if order matters, make it explicit
  (`.before`/`.after`/`.chain()`/system sets).
- **States: pick the right scope.** The global app FSM is Bevy's `States`. Model a
  *per-entity* FSM as one enum component per machine (not a component per state)
  with transition systems.
- **Relationships gotcha.** `Transform` propagation is automatic only for the
  parent/child (`ChildOf`) relationship. Custom relationships don't get it — wire
  any derived propagation yourself.
- **Minimize archetype moves.** Avoid churny component add/remove; prefer
  `Option<T>` fields or an enum for togglable state; profile fragmentation on hot
  paths.
- **Prefer immutable components until you can't.** Default a component to
  `#[component(immutable)]` — it documents intent, lets the type enforce its own
  invariants, and guarantees lifecycle hooks (`on_insert`/`on_replace`) run on
  every change. Drop to mutable only when immutability is genuinely *unavailable
  for the job*, which in practice means **state that changes often, especially
  per-frame** (`Transform`, `Health`, `Velocity`, timers): an immutable component
  can only change by remove-and-reinsert, so mutating it hot triggers an archetype
  move every time. Rule of thumb: **set-once / rarely-changed / invariant-bearing →
  immutable; mutated each tick → mutable.**
- **Static data.** Game data lives in `const` arrays/tables, applied via systems.
- **Store intent, derive results late.** Keep the *authored source of truth* in
  components (the input, the desired action, the recorded command) and recompute
  derived values in systems — don't bake computed results into stored state where
  they can desync from their inputs. (Central to arenic's record/replay: store the
  command stream, derive transforms on playback.)

## Query & scheduling efficiency

A query's signature is a **scheduling contract**: Bevy parallelizes by declared
access, not by what a system happens to touch at runtime. Shape queries so the
scheduler can do its job. (Run-condition gating and batch-over-lookup iteration
are covered above — these rules are about the query shape itself.)

- **Fetch only what you need.** Every component in the data tuple is fetched per
  iteration and widens the access contract. A marker you only test for presence
  belongs in the filter (`Query<&Transform, With<Selected>>`), never in the data
  (`Query<(&Transform, &Selected)>`).
- **`&mut T` only when the system actually mutates T.** Mutable access serializes
  the system against every other reader/writer of T, and is where spurious
  change ticks come from. If only a subset mutates, filter to that subset
  instead of branching over a broad `&mut` query.
- **Filter before branching.** Prefer a `With<T>`-narrowed system over fetching
  `Has<T>` and branching in the loop body. `Has<T>` is for the genuine case of
  ONE system whose per-entity behavior forks (e.g. `move_selected`'s
  live-vs-ghost fork). `Option<&T>` additionally **widens** the matched set —
  use it only when the shape really is "this base set, maybe with extra data",
  never as a lazy substitute for a second, narrower system.
- **`Changed<T>` is a deref flag, not a value diff.** It fires on every
  `DerefMut` (and on add) — Bevy never compares old vs new. A system that
  unconditionally writes re-dirties the world every frame and defeats every
  downstream `Changed<T>` / `resource_changed` gate. Write-if-changed
  (`set_if_neq`, or compare before assigning through `Mut`) is the discipline
  that makes change-detection pipelines actually skip work.
- **Iterate the smaller set.** When scoping work to one parent, a narrow marker
  query filtered by `child_of.parent() == target` (tens of ghosts) beats walking
  the parent's `Children` (thousands of tiles) and probing each — and the
  reverse holds when the child list is the small side. Pick by cardinality, not
  by idiom.
- **Default (table) storage until measured.** Table storage iterates fast;
  sparse-set trades iteration speed for cheap insert/remove. Don't reach for
  `#[component(storage = "SparseSet")]` without a profile showing real
  add/remove churn.
- **Split systems by job, not into dust.** One system per behavior (capture,
  movement, playback, HUD) — but ten fragments all mutating the same component
  just force explicit ordering and serialize anyway. A good system reads: "when
  this condition holds, operate on exactly this set of entities."

## Determinism (record/replay)

Arenic is built around recording timelines and replaying them, so simulation code —
as it lands — must be **bit-stable across builds and frame rates**. This is the
"Predictable & Deterministic" value made concrete (and applies pre-emptively to grid
and tick math today, even before a replay system consumes it):

- **Sim on the fixed timestep.** Gameplay that is recorded runs in `FixedUpdate`
  (`Time<Fixed>`); don't let a recorded quantity depend on per-frame `delta_secs()`
  — variable frame rate would desync the replay.
- **Explicit overflow policy on every sim number.** No bare `+`/`-`/`*` in tick,
  grid, or accumulator math — `strict_*` (fail loud) or `wrapping_*` (wrap stably).
  See the Hard-requirements gate.
- **No nondeterministic inputs in the sim.** No wall-clock, no unseeded RNG (seed
  from the run/tick), and **no reliance on `HashMap` iteration order** — sort, or
  use an ordered structure, when order affects outcome.
- **Replay = pure function of (recorded input, tick).** Re-derivation on playback
  must not read live state that wasn't recorded. Keep transition handlers
  **idempotent** so a replayed event reproduces the same state.
- **`Copy` ranges in recorded data.** Store a span in a component/resource as
  `core::range::Range` (it's `Copy`), not `a..b` (legacy range isn't `Copy` and
  blocks `#[derive(Copy)]`).

## Rust 2024 / nightly idioms

Edition-2024, nightly-toolchain idioms that materially improve correctness or
perf for this game. (None of these are Bevy APIs, so none live in the skill;
where one corresponds to a stdlib rename, use it by its current name.)

- **Let chains.** Flatten nested `if let { if let { if cond } }` into
  `if let A && let B && cond` — the `-D warnings` gate fires `collapsible_if` on
  the nesting, and chaining is the readable way to gate a system on several
  `Option`/component lookups.
- **`if let` guards in `match`.** A fallible secondary lookup goes in the arm
  (`v if let Some(y) = f(v) => …`), not a nested `if let` in the body.
- **RPIT `+ use<>`.** A helper returning `impl Iterator`/`impl Fn` that does **not**
  borrow `&self`/query data must add `+ use<>` — edition 2024 captures all in-scope
  lifetimes by default, silently chaining the result to the borrow so callers can't
  touch anything else until it's dropped. Add only the params you truly borrow.
- **Temporary scopes.** Don't assume a guard/borrow created in an `if let`
  scrutinee or a block's tail expression survives to end-of-block — 2024 drops it
  sooner. Bind it with `let` if you need it held.
- **`get_disjoint_mut`.** `&mut` to two-plus elements of the same slice/`Vec`/
  `HashMap` by index (swapping tiles, updating paired entities) — not repeated
  indexing (borrow-checker forbidden) or `unsafe`/`split_at_mut` workarounds.
- **`extract_if`.** Remove-and-collect the matching elements of a `Vec` (expired
  effects, dead entities in a side list) — `retain` discards them and a manual
  `swap_remove` loop reorders and is off-by-one-prone.
- **`as_chunks::<N>()`.** Fixed-size slice groups (RGBA pixels, N verts/quad) —
  the group size is in the type and the remainder is handled for you.
- **`[x; _]`.** Inferred array length in a body when a `const` or the return type
  already fixes it — don't hard-code the count twice (it drifts when the grid
  resizes). Not allowed in signatures.
- **`LazyLock`.** Compute-once global tables (lookup/config built once) — not
  hand-rolled `OnceLock` + init checks; it's `Sync` across Bevy's threads.
- **`core::hint::cold_path()`.** Mark the rare branch (error/spawn/despawn) at the
  top of a hot per-entity/per-tick loop to keep the common path straight-line.
- **`gen {}` blocks (nightly).** Lazily yield a sequence — procgen, spatial/AoE
  ring scans, streaming spawns — before hand-writing an `Iterator` impl or
  `from_fn` (the manual state machine is the usual off-by-one source). Needs
  `#![feature(gen_blocks)]`.
- **Don't.** No `async` in ECS systems (Bevy schedules synchronously — keep async
  at the asset-loader / task-pool boundary). Never `generic_const_exprs` or
  `specialization` — both are unsound/incomplete; redesign with a plain `const`,
  an enum, or a trait method.

## Scheduling & events

- **Group systems into named phase sets.** Use `#[derive(SystemSet)]` for ordering
  phases that mirror the Data-flow loop (Read → Process → Write, or
  Input → Sim → Render). Name sets by *behavior* (`Movement`, `Damage`), not
  implementation.
- **Run conditions, not guard-and-return.** Gate a system with `.run_if(..)`
  (`in_state(S)`, `resource_exists::<T>`, a custom predicate) instead of an early
  `return` at the top of the body — the scheduler can skip the system entirely and
  the intent stays declarative.
- **Centralize state transitions.** Prefer one handler/observer that consumes a
  change and emits its follow-ups over the same mutation scattered across many
  systems — easier to reason about and to keep deterministic. Make that handler
  idempotent: safe to skip, or to run twice, with the same result.

## Naming & API surface

- **Components are plain nouns** — `Health`, `Velocity`, `Boss`, `Theme`. Do
  **not** suffix with `Component`/`Tag`/`Data`; `HealthComponent` / `HealthData` is
  an anti-pattern in Bevy and in this repo. Only add a disambiguating name when two
  types for one concept genuinely coexist (component `Health` vs message
  `HealthChanged` / `Damage`).
- **One interface per concept.** Expose only the variant that serves the API's
  purpose; keep internal/intermediate forms private unless they're genuinely
  distinct concepts. (Ties to CLAUDE.md's "import from the front door".)
- **Import at module level.** Bring enum variants, associated constants, and common
  types in at the top; avoid inline qualified paths. Reach for a module's
  re-exported surface, not deep `crate::a::b::Type` paths across boundaries.
- **Public fields, no getters/setters.** This is a solo/indie codebase — fields are
  `pub` and mutated directly. A `fn health(&self) -> u32 { self.0 }` / `set_health`
  pair that just forwards to a field is needless ceremony; delete it. Don't reach
  for `private` "encapsulation" by default. The *only* time a field stays private +
  accessor-guarded is when there's a **real invariant** the type must enforce (a
  value that must stay normalized; or use `#[component(immutable)]` instead of a
  hand-written setter). No invariant → `pub` field, direct access.
- **Finite sets are enums.** Fewer than ~20 compile-time-known values → an enum;
  prefer `match` over `if let` chains for exhaustiveness. Avoid a catch-all `_` arm
  where you can name the variants — it silently swallows variants added later.
- **Consume `self` for one-way transforms.** When data flows one direction (a
  builder finalizing, a conversion that shouldn't leave the source usable), take
  `self` by value rather than `&self` + clone. Let the type system enforce it.
- **`Display`/`FromStr` at human-readable boundaries.** No internal-representation
  leakage.

## Rust craftsmanship

- **Return views, not copies.** Hand back `impl Iterator` or a slice rather than
  allocating a `Vec`; if a function must allocate, say so in its doc.
- **No global mutable state.** Use ECS **Resources** for shared mutable state and
  **Components** for per-entity state — never `static mut` or ad-hoc singletons.
- **Design for idempotency.** Operations give the same result applied multiple
  times; property-test mathematical components where it pays off.
- **Docs are tests.** Rustdoc examples compile; prose is terse but grammatical
  (articles and punctuation).

## Testing

- **Test systems in isolation.** Minimal worlds, known entity configurations, mock
  messages/resources, property-test math. `world.run_system_once(..)` returns a
  `Result` — handle it.
- **`assert_matches!` over `assert!(matches!(..))`** — it prints the actual value on
  failure instead of just `false`.

## Guidelines (smells, not tripwires)

- **Single responsibility per system.** One job per system. Treat systems over
  ~50 LOC as a prompt to look for a split, not a hard cap — linear setup/spawn
  systems can legitimately run long.
- **Direct mutation is the happy path.** Most state changes are `Query<&mut T>` —
  that's idiomatic. Route mutations through messages/events only as a deliberate
  pattern for cross-cutting concerns (damage, networking), not as a blanket "all
  mutations flow through events."
- **Build for now.** Build exactly what today needs — don't pre-build for tomorrow.
