---
name: bevy-018
description: >-
  Bevy 0.18 modern-API ruleset. CONSULT BEFORE writing, editing, or reviewing ANY
  Bevy (bevy / bevy_*) Rust code in this repo. Maps every deprecated or removed
  pre-0.18 API to its single current 0.18 form: required components (no *Bundle),
  the Message vs Event split, On<E> observers, ChildOf hierarchy, Single / single()
  -> Result, srgb colors, ButtonInput, Time<Fixed>, and more. Trigger on any Bevy
  type, component, system, query, schedule, asset, or plugin.
---

# Bevy 0.18 — Modern API Only

This project pins **`bevy = "0.18.1"`** (edition 2024). Emit **ONLY** Bevy 0.18 APIs.
Most pre-0.18 code an LLM "remembers" is wrong. When in doubt, find the symbol below.

## How to read this file
- Each entry is `OLD -> NEW — note`. **Left of `->` is FORBIDDEN** (removed/renamed). **Right is the only correct 0.18 form.**
- Code fences show the canonical 0.18 pattern. Prefer them verbatim.
- If a symbol isn't listed, it's likely unchanged — but verify against <https://docs.rs/bevy/0.18.1/bevy/> and the prelude <https://docs.rs/bevy/0.18.1/bevy/prelude/index.html>.

## Non-negotiable rules
1. **No `*Bundle` types.** They are all deleted. Spawn components directly; required components auto-insert the rest. `spawn((A, B, C))` — a tuple IS the bundle.
2. **No bare `Handle<T>` as a component.** Use the wrapper: `Mesh3d`, `Mesh2d`, `MeshMaterial3d`, `MeshMaterial2d`, `SceneRoot`, `AudioPlayer`.
3. **Buffered events are `Message`** (`MessageReader`/`MessageWriter`/`write`/`read`). `#[derive(Event)]` + `On<E>` is for **observers only**; targeted observer events use `#[derive(EntityEvent)]`.
4. **Observers take `On<E>`, never `Trigger<E>`.** Lifecycle events are `Add`/`Insert`/`Replace`/`Remove`/`Despawn` (no `On` prefix on the type).
5. **`add_systems(Schedule, ..)` always** — schedule is explicit, never implied; no `.system()`, no `add_system`, no stages.
6. **Colors are sRGB-explicit**: `Color::srgb(..)`, not `Color::rgb(..)`. Palette consts live in `bevy::color::palettes::css`.
7. **`Query::single()`/`single_mut()` return `Result`** — handle with `?` (systems are fallible) or use the `Single<&T>` param. `get_single` is deprecated.
8. **Hierarchy is `ChildOf` + `Children`**: build with the `children![..]` macro; read parent via `child_of.parent()`. `despawn()` is recursive by default.

## Top reflexes (most-corrected APIs)
- `SpriteBundle`/`Camera2dBundle`/`TextBundle`/`NodeBundle`/`PbrBundle` -> `Sprite`/`Camera2d`/`Text`/`Node`/`Mesh3d`+`MeshMaterial3d`
- `Color::rgb` -> `Color::srgb`  •  `Input<KeyCode>` -> `ButtonInput<KeyCode>`  •  `KeyCode::W` -> `KeyCode::KeyW`
- `EventReader/EventWriter` -> `MessageReader/MessageWriter`  •  `EventWriter::send` -> `MessageWriter::write`
- `Trigger<E>` -> `On<E>`  •  `app.observe`/`world.observe` -> `add_observer`
- `add_system`/`add_startup_system` -> `add_systems(Update, ..)`/`add_systems(Startup, ..)`
- `Parent`/`with_children` -> `ChildOf`/`children![..]`  •  `despawn_recursive` -> `despawn`
- `time.delta_seconds()` -> `time.delta_secs()`  •  `Res<Windows>` -> `Query<&Window, With<PrimaryWindow>>`
- `shape::Cube`/`shape::*` -> `Cuboid` and other `bevy_math` primitives  •  `OnUpdate(S)` -> `.run_if(in_state(S))`

---

## ECS: Components & Bundles (Required Components)

### Defining Components & Resources
- `struct Foo;` (implicit `Component`) -> `#[derive(Component)] struct Foo;` — derive required
- `impl Component { type Storage = TableStorage; }` -> `#[derive(Component)] #[component(storage = "SparseSet")] struct Foo;` — storage on derive (`"Table"`/`"SparseSet"`)
- `struct R;` (implicit `Resource`) -> `#[derive(Resource)] struct R;` — opt-in derive
- `#[derive(Resource)] struct R<'a>{..}` -> `Resource` must be `'static` — non-static lifetime is a compile error
- `bevy::ecs::system::Resource` -> `bevy::ecs::resource::Resource` — module moved (prelude still works)
- mutable-only -> `#[component(immutable)]` — opt-in immutable components

### Required Components (bundles are gone)
Spawn the marker component; required components auto-insert. Set defaults with `#[require(..)]`.
```rust
#[derive(Component)]
#[require(Transform, Visibility, Name = Name::new("x"))]
struct Player;
commands.spawn((Player, Transform::from_xyz(0., 0., 0.)));
```
- `#[require(A(returns_a))]` -> `#[require(A = returns_a())]` — Rust expression syntax
- `#[require(A(|| A(10)))]` -> `#[require(A = A(10))]` — inline value

### Spawning, Insert, Remove
- `spawn().insert_bundle((A,B))` / `spawn_bundle((A,B))` -> `spawn((A, B))` — tuple is a bundle
- `.insert_bundle(x)` -> `.insert(x)`; `.remove_bundle::<B>()` -> `.remove::<B>()`
- `.remove_bundle_intersection::<B>()` -> `.remove_intersection::<B>()`
- `#[derive(Bundle)]` field `#[bundle]` attr -> drop it — nested bundles need no attr

### Bundle Replacements (use components/wrappers)
- `SpriteBundle{..}` -> `Sprite`
- `Camera2dBundle` / `Camera3dBundle` -> `Camera2d` / `Camera3d`
- `NodeBundle` / `ButtonBundle` / `ImageBundle` -> `Node` / `Button` / `ImageNode`
- `TextBundle` / `Text2dBundle` -> `Text` / `Text2d` (+ `TextFont` + `TextColor`)
- `PointLightBundle` / `DirectionalLightBundle` / `SpotLightBundle` -> `PointLight` / `DirectionalLight` / `SpotLight`
- `PbrBundle` / `MaterialMeshBundle<M>` -> `Mesh3d(mesh)` + `MeshMaterial3d(material)`
- `SceneBundle{scene}` -> `SceneRoot(handle)`
- `TransformBundle` / `SpatialBundle` / `VisibilityBundle` -> `Transform` / `Visibility` (others auto-required)
- `AudioBundle` / `AudioSourceBundle` / `PitchBundle` -> `AudioPlayer(handle)`
- `UiMaterialBundle<M>` -> `MaterialNode<M>`

### Handles Are Wrapper Components
Bare `Handle<T>` is no longer a component.
- `Handle<Mesh>` -> `Mesh3d(h)` or `Mesh2d(h)`
- `Handle<StandardMaterial>` -> `MeshMaterial3d(h)`
- `Handle<ColorMaterial>` / `Mesh2dHandle` -> `MeshMaterial2d(h)` / `Mesh2d(h)`
- `Handle<Scene>` -> `SceneRoot(h)`

### Visibility (enum, not bool)
- `Visibility::VISIBLE` -> `Visibility::Inherited`
- `Visibility::INVISIBLE` -> `Visibility::Hidden`
- `visibility.is_visible` -> `*visibility == Visibility::Inherited` — compare enum

### Removal Detection & Lifecycle
- `for e in &removed` (direct iter) -> `for e in removed.read()` (`mut RemovedComponents<T>`)
- `RemovedComponents::iter()` / `iter_with_id()` -> `.read()` / `.read_with_id()`
- `bevy_ecs::removal_detection::*`, world events `Add/Insert/Remove/Replace/Despawn`, `HookContext` -> `bevy_ecs::lifecycle::*` — reorg

### Hooks & Change Detection
- `Component::register_component_hooks(&mut ComponentHooks)` -> `fn on_add() -> Option<ComponentHook>` (also `on_insert`/`on_replace`/`on_remove`)
- `fn hook(world, entity, component_id)` -> `fn hook(world, HookContext { entity, component_id, caller })`
- `set_if_neq()` returns `()` -> returns `bool` (`true` if changed) — note `State<T>` is read-only (`state.get()`, no setter); the always-transition behavior is on `NextState::set` (see States)
- `ChangedRes<R>` -> `Res<R>` + `if res.is_changed() {}` — `ChangedRes` removed
- `component::{Tick, ComponentTicks, CheckChangeTicks}`, `TickCells` -> `change_detection::{..}`, `ComponentTickCells`

### World & Bundle API
- `World::init_component/init_bundle/init_resource` -> `register_component/register_bundle/register_resource`
- `world.flush_commands()` -> `world.flush()`
- `ComponentTicks::last_changed_tick()/added_tick()` -> fields `.last_changed_tick` / `.added_tick`
- `Bundle::component_ids(comps, &mut FnMut)` -> `component_ids(comps) -> impl Iterator<Item=ComponentId>` (caller `.collect()`)

## ECS: Systems, Schedule, States & Plugins

### App & Plugins
Initial state from `Default`; `main` returns `AppExit`.
```rust
fn main() -> AppExit { App::new().add_plugins(DefaultPlugins).run() }
```
- `App::build()` / `AppBuilder` -> `App::new()` — `App` is the only builder type
- `Plugin::build(&self, app: &mut AppBuilder)` -> `Plugin::build(&self, app: &mut App)` — no `AppBuilder`
- `app.add_plugin(p)` -> `app.add_plugins(p)` — one method, one-or-many
- `app.add_resource(r)` -> `app.insert_resource(r)` — naming consistency
- `app.world` (field) -> `app.world()` / `app.world_mut()` — methods, not a field
- `AppExit` (unit) -> `AppExit::Success` / `AppExit::Error(NonZeroU8)` — now an enum
- `.add_before::<P, _>(Foo)` -> `.add_before::<P>(Foo)` — second generic dropped
- `GLOBAL_ERROR_HANDLER` -> `app.set_error_handler(h)` or `DefaultErrorHandler(h)` resource
- Dynamic plugins (`bevy_dynamic_plugin`, `DynamicPlugin`) -> REMOVED — use static `add_plugins`

Configure plugins via `.set()`, not resources:
```rust
DefaultPlugins.set(WindowPlugin { primary_window: Some(Window { .. }), ..default() })
DefaultPlugins.build().disable::<LogPlugin>()
```
- `LogSettings` / `ImageSettings` / `AssetServerSettings` -> `LogPlugin` / `ImagePlugin` / `AssetPlugin` via `.set()`
- `add_plugins_with(P, |g| g.disable::<X>())` -> `DefaultPlugins.build().disable::<X>()`
- Cargo features: `animation`->`gltf_animation`, `bevy_sprite_picking_backend`->`sprite_picking`, `bevy_ui_picking_backend`->`ui_picking`, `bevy_mesh_picking_backend`->`mesh_picking`; collections `2d`/`3d`/`ui`/`default_app`/`default_platform`

### Schedule & Systems
Schedule is first arg; `Update` never implied; no `.system()`.
```rust
app.add_systems(Startup, setup)
   .add_systems(Update, (move_player, check).chain())
   .add_systems(FixedUpdate, physics);
```
- `app.add_system(a)` -> `app.add_systems(Update, a)` — schedule explicit
- `app.add_startup_system(a)` / `.on_startup()` -> `app.add_systems(Startup, a)`
- `add_system_to_stage(CoreStage::PostUpdate, s)` -> `app.add_systems(PostUpdate, s)`
- `CoreStage` / `CoreSet` / base sets -> schedules: `Update`, `PostUpdate`, `PreUpdate`, `Startup`, `PreStartup`, `PostStartup`, `FixedUpdate`
- `sys.system()` / `sys.exclusive_system()` -> `sys` — implements `System` directly
- `.label(L)` / `#[derive(SystemLabel)]` -> `.in_set(S)` / `#[derive(SystemSet)]`
- `configure_set(A.in_schedule(PostUpdate)...)` -> `configure_sets(PostUpdate, A.after(B))` — schedule first, plural
- `apply_system_buffers` / `apply_deferred()` -> `ApplyDeferred` — ZST type, not a fn
- `.chain(handler)` -> `.pipe(handler)` — system piping
- `Condition` trait -> `SystemCondition`
- `SimpleExecutor` -> `SingleThreadedExecutor` / `MultiThreadedExecutor` — removed; use `.chain()` for ordering
- `world.get_resource::<T>().unwrap()` -> `world.resource::<T>()` — infallible getter
- `QuerySet<(..)>` (`.q0()`) -> `ParamSet<(..)>` (`.p0()`) — any conflicting params
- `world.run_system_once(s)` -> returns `Result<Out, _>` — validate `?`/`unwrap`
- Systems are fallible: return `()` or `Result<(), BevyError>`; use `?` inside

### Run Conditions
Conditions are systems — NO parentheses.
```rust
app.add_systems(Update, ui.run_if(in_state(Menu).and(resource_exists::<Config>)));
```
- `.with_run_criteria(c)` (`ShouldRun`) -> `.run_if(cond)` — returns `bool`
- `resource_exists::<T>()` -> `resource_exists::<T>` — drop parens (also `resource_added`/`resource_changed`/`state_exists`/`any_with_component`)
- `run_once()` -> `run_once` — no parens
- `.and_then(o)` / `.or_else(o)` -> `.and(o)` / `.or(o)`

### States
States live in `bevy::state`; transitions queued via `NextState`.
```rust
#[derive(States, Default, Debug, Clone, PartialEq, Eq, Hash)]
enum AppState { #[default] Menu, Game }
app.init_state::<AppState>()
   .add_systems(OnEnter(AppState::Game), spawn_level);
fn go(mut next: ResMut<NextState<AppState>>) { next.set(AppState::Game); }
```
- `App::add_state::<S>()` -> `App::init_state::<S>()` — needs `use bevy::state::app::AppExtStates as _;` if not via prelude
- `State<T>.0` -> `state.get()` — inner field private
- `NextState(Some(s))` / `NextState(None)` -> `NextState::Pending(s)` / `NextState::Unchanged`
- `SystemSet::on_enter/on_update/on_exit(S)` -> `OnEnter(S)` / `.run_if(in_state(S))` / `OnExit(S)` schedules
- `OnUpdate(S)` -> `.run_if(in_state(S))` — set removed
- `StateScoped` / `clear_state_scoped_entities` -> `DespawnOnExit` / `despawn_entities_on_exit_state` (+ `DespawnOnEnter`)
- `enable_state_scoped_entities` / `#[states(scoped_entities)]` -> REMOVED — always on
- `StateTransitionEvent { before, after }` -> `{ exited, entered }`
- `States::variants()` -> REMOVED — use `strum` or similar
- `next.set(s)` now fires even if same state -> use `next.set_if_neq(s)` for skip-if-equal (0.18)

[migration guide](https://bevy.org/learn/migration-guides/0-17-to-0-18/)

## ECS: Queries, Entities, Commands, Observers & Relationships

### Spawning & Commands
Modern spawn folds bundles back into `spawn(bundle)`; `Commands` is by-value.
```rust
fn sys(mut commands: Commands) {
    let e = commands.spawn((Transform::default(), Player)).id();
    commands.entity(e).insert(Health(100)).remove::<Player>();
}
```
- `commands: &mut Commands` -> `mut commands: Commands` — by-value `SystemParam`
- `commands.spawn().insert_bundle(b)` / `spawn_bundle(b)` / `.with(c)` -> `commands.spawn(bundle)` — unified tuple bundle
- `commands.spawn().id()` -> `commands.spawn_empty().id()` — empty spawn renamed
- `commands.insert_one(e, C)` / `remove_one::<C>(e)` -> `commands.entity(e).insert(C)` / `.remove::<C>()`
- `.current_entity().unwrap()` -> `.id()` — entity id getter
- `commands.add(cmd)` / `commands.push(cmd)` -> `commands.queue(cmd)` — deferred API renamed
- `commands.register_one_shot_system(s)` -> `commands.register_system(s)`
- `commands.queue(SendEvent { e })` -> `commands.write_event(e)` — `send_event` deprecated for events
- `commands.queue(TriggerEvent {..})` -> `commands.trigger(event)` — per-entity, no multi-target
- `Command::write()` / `EntityCommand::write()` -> `::apply()`
- `use bevy::ecs::system::{Command, CommandQueue}` -> `use bevy::ecs::world::{Command, CommandQueue}`
- `world.insert_non_send(v)` -> `world.insert_non_send_resource(v)`

### Entities
- `Entity::id()` -> `Entity::index()`; numeric form -> `Entity::index_u32()`
- `Entity::from_raw(u32)` -> `Entity::from_raw_u32(u32) -> Option<Entity>` or `Entity::from_index(EntityIndex)`
- `Entity::from_row()` / `row()` -> `Entity::from_index()` / `index()` — `EntityRow` is now `EntityIndex`
- `entity_mut.remove::<C>()` (returning value) -> `entity_mut.take::<C>() -> Option<C>`
- `world.spawn().id()` -> `world.spawn_empty().id()`
- `World::get_entity(e)` -> `Result<EntityRef, _>`; use `.ok()` for `Option`
- `World::get_entity_mut(e)` -> `Result<EntityMut, _>`; `.ok()` for old behavior
- `world.many_entities([..])` / `get_many_entities(..)` -> `world.entity([..])` / `world.get_entity(..)`
- `world.iter_entities()` -> `world.query::<EntityRef>().iter(&world)`
- `World::inspect_entity(e)` -> `Result<impl Iterator<Item=&ComponentInfo>, _>`
- `EntityClonerBuilder` `allow_all().deny::<C>()` -> `EntityCloner::build_opt_out(&mut world).deny::<C>()` (or `build_opt_in().allow::<C>()`)
- `Entities::alloc/free` -> `world.entities_allocator_mut().alloc/free/alloc_many` (mutating); `entities_allocator()` is the read accessor; new `World::spawn_at()`

### Queries
- `Query::single()` / `single_mut()` -> return `Result` (no longer panic); `get_single`/`get_single_mut` deprecated
- `Query<&A, &mut B>` (component filter) -> `Query<&A, With<B>>` — filters must be `QueryFilter`
- `Query::get_component::<T>(e)` -> `Query::get(e)` then index the tuple item
- `Option<With<T>>` in data -> `Has<T>` — boolean presence
- `Query::par_for_each` / `for_each_mut` -> `query.par_iter().for_each(|i| {})`
- `ChangeTrackers<T>` -> `Ref<T>` for change detection in queries
- `Query::to_readonly()` -> `Query::as_readonly()`
- `Query::many()` / `many_mut()` -> `get_many()` / `get_many_mut()`
- `Option<Single<&T>>` -> `Query<&T>` + `query.single()` — `Option<Single>` is `None` on multiple matches now
- `Query<EntityRef>` -> `Query<EntityRef, Allow<Disabled>>` — no longer bypasses default filters
- `#[derive(WorldQuery)]` -> `#[derive(QueryData)]`; `ReadOnlyWorldQuery` -> `QueryFilter`
- `#[world_query(..)]` -> `#[query_data(..)]` / `#[query_filter(..)]`
- `QuerySet<(Query<..>,)>` -> `ParamSet<(Query<..>,)>` — conflicting queries use `ParamSet`

### Relationships & Hierarchy
`Parent` is now `ChildOf` (tuple struct); set via plain insertion.
```rust
commands.entity(child).insert(ChildOf(parent));
let p: Entity = child_of.parent(); // not Deref/.get()/.0
// idiomatic spawn-time hierarchy:
commands.spawn((Player, children![(Child1,), (Child2,)]));
```
- `.with_children(|p| { p.spawn(C); })` / `push_children(&[..])` -> `children![(C,), ..]` spawn macro (or `related![Rel, ..]` for custom relationships) — preferred hierarchy builder
- `Parent` component -> `ChildOf(parent)`; read with `child_of.parent()`
- `entity.set_parent(p)` -> `entity.insert(ChildOf(p))`
- `entity_commands.push_children(&[..])` -> `.add_children(&[..])`
- `commands.queue(AddChild {..})` -> `commands.entity(parent).add_child(child)`
- `with_children(|cb: &mut ChildBuilder|..)` -> `|cb: &mut ChildSpawnerCommands|..`; `parent_entity()` -> `target_entity()`
- `clear_children()` / `remove_children()` / `remove_child()` -> `detach_all_children()` / `detach_children()` / `detach_child()` — don't despawn
- `despawn_descendants()` / `clear_related()` -> `despawn_related::<Children>()` / `detach_all_related()`
- `HierarchyQueryExt::parent()` / `children()` -> `Query::related()` / `relationship_sources()`

### Despawning
- `entity.despawn_recursive()` -> `entity.despawn()` — recursive by default
- `commands.queue(DespawnRecursive { warn: false })` -> `commands.entity(e).try_despawn()`

### Observers & Events
`Event` implies `Component`; `Trigger` is now `On`; lifecycle events dropped the `On` prefix.
```rust
#[derive(Event)] struct Explode { entity: Entity }
app.add_observer(|add: On<Add, Player>| { let e = add.entity; });
```
- `#[derive(Event, Component)]` -> `#[derive(Event)]` — `Component` implied
- `Trigger<E>` -> `On<E>`; `Trigger<OnAdd, T>` -> `On<Add, T>` (`OnAdd/OnInsert/OnReplace/OnRemove/OnDespawn` -> `Add/Insert/Replace/Remove/Despawn`)
- `trigger.target()` -> `event.entity` (the `EntityEvent` field) — `On::target()` removed
- `add.components()` -> `add.trigger().components` — lifecycle component access
- `app.observe(..)` / `world.observe(..)` / `commands.observe(..)` -> `add_observer(..)`
- `entity.observe_entity(..)` -> `entity.observe(..)`
- `Observer<A, B>` / `ObserverState` -> `Observer` — unified, no type params
- `world.trigger_targets(E, [e1, e2])` -> per-entity `world.trigger(E { entity })` — no multi-target
- `#[derive(Event)] #[event(traversal=&'static ChildOf)]` -> `#[derive(EntityEvent)] #[entity_event(propagate)]` — propagation needs `EntityEvent` ([guide](https://bevy.org/learn/migration-guides/0-16-to-0-17/))

## Events & Messages

In 0.17+ "buffered events" became **Messages**; `Event` is now only for observers. Two distinct systems.

### Buffered events are now Messages
- `#[derive(Event)]` (buffered) -> `#[derive(Message)]` — buffered events renamed to messages
- `EventReader<E>` -> `MessageReader<M>` — direct `SystemParam`; must be `mut` to `.read()`/`.clear()`
- `EventWriter<E>` -> `MessageWriter<M>` — direct `SystemParam`
- `Events<E>` -> `Messages<M>` — collection type renamed
- `EventId` / `SendBatchIds` -> `MessageId` / `WriteBatchIds` — id types renamed
- `ManualEventReader` / `EventCursor` -> `MessageCursor` — was `ManualEventReader`->`EventCursor` in 0.15
```rust
#[derive(Message)] struct Hit { dmg: u32 }
fn send(mut w: MessageWriter<Hit>) { w.write(Hit { dmg: 5 }); }
fn recv(mut r: MessageReader<Hit>) { for h in r.read() { /* … */ } }
```
[docs](https://docs.rs/bevy/0.18.1/bevy/ecs/message/index.html)

### Reading messages
- `reader.iter()` / `iter_with_id()` -> `r.read()` / `r.read_with_id()` — renamed
- `for e in &mut reader` -> `for e in r.read()` — `IntoIterator` removed
- `events.get_reader()` / `get_cursor()` / `ManualEventReader` -> `Messages::get_cursor()` -> `MessageCursor` — old getters removed
- `events.oldest_id()` -> `messages.oldest_message_count()` — semantics rename

### Writing messages
- `EventWriter::send()` -> `MessageWriter::write()` — returns `MessageId` (ignore with `;`)
- `send_batch()` / `send_default()` -> `write_batch()` / `write_default()` — renamed
- `Events::send/send_default/send_batch` -> `Messages::write/write_default/write_batch` — on collection
- `World::send_event*` / `Commands::send_event` / `DeferredWorld::send_event*` -> `write_message*` / `Commands::write_message` / `write_message*` — renamed
- `world.insert_resource(Events::<E>::default())` -> `world.add_message::<M>()` — registration helper
- `RemovedComponents::events` / `RemovedComponentEvents` -> `RemovedComponents::messages` / `RemovedComponentMessages` — split rename

### Observer events (Event / EntityEvent)
`Event` now means "observer event"; targeted events use `EntityEvent`.
- `#[derive(Event)] + world.trigger_targets(E, entity)` -> `#[derive(EntityEvent)]` + `world.trigger(E { entity })` — `trigger_targets` removed; needs `entity: Entity` field
- observer `Trigger<E>` / `On<E>` -> `On<E>` — observer system param
- `EntityEvent::from()` / `event_target_mut()` -> `SetEntityEventTarget` trait — entity events immutable by default; mutable opt-in via `#[entity_event(propagate)]`
- manual `impl Event` -> add `type Traverse = TraverseNone; const AUTO_PROPAGATE: bool = false;` — propagation items required
```rust
#[derive(EntityEvent)] struct Explode { entity: Entity }
fn on_explode(ev: On<Explode>) { let _ = ev.entity; }
world.trigger(Explode { entity });
```

### Imports / lifetimes (older footguns)
- `use bevy::app::{EventReader, EventWriter, Events, …}` -> `use bevy::ecs::message::{MessageReader, MessageWriter, Messages}` — moved out of `bevy_app`
- `EventWriter<'w, 's, T>` -> `MessageWriter<'w, M>` — unused `'s` lifetime removed

## Rendering: Camera, Lighting, Material & Mesh

### Camera spawning
Bundles are gone — spawn `Camera2d`/`Camera3d` (required components pull in `Camera`, `Transform`, etc.).
```rust
commands.spawn(Camera2d);
commands.spawn((Camera3d::default(), Transform::from_xyz(0., 0., 10.)));
```
- `Camera2dBundle`, `Camera3dBundle` -> `Camera2d` / `Camera3d` — required components, no bundles
- `PerspectiveCameraBundle` / `OrthographicCameraBundle` / `UiCameraBundle` -> `Camera2d` / `Camera3d` — all removed
- `Camera3dBundle { dither }` -> `Camera3d { deband_dither }` — field renamed
- `Camera::priority` -> `Camera.order` — field renamed
- `Camera { window }` -> `Camera { target: RenderTarget::Window(..) }` — render target
- `Camera { target: RenderTarget::Image(..) }` -> spawn `RenderTarget::Image(..)` as separate component — `RenderTarget` is now a component
- `Camera { hdr: true }` -> `Hdr` marker component (`bevy::render::view::Hdr`) — effects `#[require(Hdr)]`
- `Camera { name: Some(..) }` -> custom marker `Component` + `With<T>` query — no name strings
- `TargetCamera` -> `UiTargetCamera` — UI-specific

### Camera projection & viewport
- `OrthographicProjection::default()` -> `OrthographicProjection::default_2d()` / `::default_3d()`
- `OrthographicProjection { scale }` -> removed; use `scaling_mode`
- `OrthographicProjection { window_origin: WindowOrigin::Center }` -> `viewport_origin: Vec2::new(0.5, 0.5)`
- `OrthographicProjection { left, right, bottom, top }` -> `area: Rect::new(left, bottom, right, top)`
- `ScalingMode::Auto` -> `ScalingMode::AutoMin`; `::None` -> `::Fixed`; `::WindowSize` -> `::WindowSize(1.0)`
- `camera.world_to_screen()` -> `camera.world_to_viewport() -> Result<Vec2, _>` — now `Result`, top-left origin
- `camera.viewport_to_world()` / `_2d()` -> same, return `Result`, top-left origin
- `camera.projection_matrix` -> `camera.projection_matrix()` — getter
- `logical_viewport_rect() -> (Vec2, Vec2)` -> `-> Rect`; `physical_viewport_rect() -> URect`

### MSAA
- `Msaa { samples: 4 }` (resource) -> `Msaa::Sample4` component on the camera entity — per-camera, not global

### Background color (ClearColor)
- `ClearColor(Color)` resource — global background, still valid: `insert_resource(ClearColor(Color::srgb(r, g, b)))`
- per-camera background -> set `Camera { clear_color: ClearColorConfig::Custom(color) | ClearColorConfig::None | ClearColorConfig::Default, ..default() }` on the `Camera2d`/`Camera3d` entity

### Lighting
- `Light` / `LightBundle` -> `PointLight` (spawn the component directly)
- `app.insert_resource(AmbientLight { .. })` -> `GlobalAmbientLight { .. }` — `AmbientLight` is now a per-camera component
- `PointLight` / `SpotLight` default intensity -> `1_000_000.0`; `DirectionalLight` -> `illuminance: lux::AMBIENT_DAYLIGHT`; `AmbientLight` brightness `80.0` — physical units
- Light types moved to `bevy::light` (re-exported in prelude)

### Mesh shapes (use `bevy_math` primitives + `Mesh::from`)
```rust
let h = meshes.add(Cuboid::new(1., 1., 1.));
commands.spawn((Mesh3d(h), MeshMaterial3d(material)));
```
- `shape::Cube` / `shape::Box` -> `Cuboid`; `shape::Quad` -> `Rectangle`; `shape::Circle` -> `Circle`
- `shape::Plane` -> `Plane3d` / `Plane2d`; `Plane { subdivisions: n }` -> `Plane3d::default().mesh().subdivisions(n)`
- `shape::Icosphere` / `shape::UVSphere` -> `Sphere`; `shape::Cylinder` -> `Cylinder`; `shape::Torus` -> `Torus`
- `shape::Capsule` -> `Capsule3d` / `Capsule2d`; `shape::RegularPolygon` -> `RegularPolygon`

### Mesh data
- `mesh.set_indices(Some(..))` -> `mesh.insert_indices(..)`; `set_indices(None)` -> `remove_indices()`
- `mesh.set_attribute("Vertex_X", v)` -> `mesh.insert_attribute(ATTR, v)` — typed `MeshVertexAttribute`
- `mesh.insert_attribute()` / `attribute()` / `indices()` -> `try_insert_attribute()` / `try_attribute()` / `try_indices()` `-> Result<_, MeshAccessError>` — for render-world meshes
- `mesh.merge(other)` -> `mesh.merge(&other) -> Result<(), MeshMergeError>` — takes ref, returns Result
- Mesh `build()` methods require `MeshBuilder` trait in scope
- Mesh/`Mesh3d`/`Mesh2d` moved to `bevy::mesh` (re-exported in prelude)

### Material / shader
- `StandardMaterial.roughness` -> `.perceptual_roughness`
- `ColorMaterial` -> now has required `uv_transform` field
- `MaterialPlugin::<M> { prepass_enabled, shadows_enabled }` -> `impl Material { fn enable_prepass() -> bool; fn enable_shadows() -> bool }`
- `impl AsBindGroup { label() }` -> `fn label() -> &'static str` now required
- `#[derive(AsStd140)]` / `AsStd430` -> `#[derive(ShaderType)]`
- WGSL: `@group(1)`/`@group(2)` material bindings -> `@group(#{MATERIAL_BIND_GROUP})`; shader `const FOO = 1.0;` -> `const FOO: f32 = 1.0;`
- WGSL imports: `bevy_pbr::mesh_view_bind_group` -> `::mesh_view_bindings`; `bevy_pbr::utils::{PI, rgb_to_hsv}` -> `bevy_render::{maths::PI, color_operations::rgb_to_hsv}`

[docs](https://bevy.org/learn/migration-guides/0-17-to-0-18/)

## 2D, Sprite, UI, Text, Color & Picking

### Sprites & Texture Atlases
2D sprites use required components, not bundles.
```rust
commands.spawn((Sprite::from_image(handle), Anchor::CENTER));
commands.spawn((Sprite::from_atlas_image(img, TextureAtlas { layout, index: 0 }),));
```
- `SpriteBundle`, `SpriteSheetBundle`, `AtlasImageBundle` -> `Sprite` + `TextureAtlas` (required components) — bundles removed
- `Sprite.resize_mode`, `ColorMaterial` for color -> `Sprite.custom_size: Option<Vec2>`, `Sprite.color` — set on `Sprite`
- `TextureAtlasSprite` -> `TextureAtlas { index }` + `Sprite` — query `&mut TextureAtlas` for index
- `Sprite { anchor: Anchor::Center }`, `Anchor::Custom(v)` -> `Anchor::CENTER` (required component), `Anchor(v)` — SCREAMING_CASE consts
- `TextureAtlas::from_grid(tex, size, cols, rows)` -> `TextureAtlasLayout::from_grid(size, cols, rows, None, None)`
- `collide_aabb::collide()` -> `Aabb2d::intersects()` — bounding-volume crate
- 2D render types (`Material2d`, `MeshMaterial2d`, `Wireframe2d`) -> enable `bevy_sprite_render` feature (`bevy::sprite_render`)

### UI Nodes & Layout
`Node` holds all layout; no `Style`, no `*Bundle`.
```rust
commands.spawn((Node { width: Val::Px(100.), border_radius: BorderRadius::all(Val::Px(4.)), ..default() }, BackgroundColor(color)));
```
- `Style::size`/`position`/`gap`/`min_size` -> `Node { width, height, left, top, row_gap, min_width, .. }` — flattened onto `Node`
- `NodeBundle`/`ButtonBundle`/`ImageBundle` -> `Node` + required components (`BackgroundColor`, etc.)
- `UiColor`/`Rect`/`Margins`/`Size`/`Direction` -> `BackgroundColor`/`UiRect`/`UiRect`/(use Node axes)/`FlexDirection`
- `Val::Undefined` -> `Val::Px(0.)` (size) or `Val::Auto` (min/max/position)
- `BorderRadius` (component) -> `Node { border_radius, .. }` field — no longer standalone
- `BorderColor(c)` -> `BorderColor::all(c)` — per-edge fields now
- `UiImage`, `UiImageSize` -> `ImageNode`, `ImageNodeSize` — `TextureAtlas` lives inside `ImageNode`
- `UiScale { scale }` -> `UiScale(1.0)`; `ScrollPosition { offset_x, offset_y }` -> `ScrollPosition(Vec2)`
- `Node` required `Transform`/`GlobalTransform` -> `UiTransform`/`UiGlobalTransform` — `UiTransform { translation: Val2, rotation: Rot2, scale: Vec2 }`
- `UiCameraConfig` -> use `Visibility` / `UiTargetCamera` — removed
- `Overflow::Hidden` -> `Overflow::clip()`; `BorderRect { left, right, top, bottom }` -> `BorderRect { min_inset: Vec2, max_inset: Vec2 }`
- UI render types (`UiMaterial`, `MaterialNode`) -> enable `bevy_ui_render` (`bevy::ui_render`)

### Text
`TextStyle` is split; sections are child `TextSpan` entities.
```rust
commands.spawn((Text::new("Hi"), TextFont { font, font_size: 24.0, ..default() }, TextColor(color), TextLayout::new_with_justify(Justify::Center)));
```
- `TextStyle { font, font_size, color }` -> `TextFont { font, font_size }` + `TextColor` — separate components
- `TextSection`/`Text::from_sections` -> `Text` + child entities with `TextSpan`
- `Text2dBundle`/`TextBundle` -> `Text` / `Text2d` + required components
- `TextAlignment` -> `JustifyText` -> `Justify` — final name `Justify` is a FIELD of the `TextLayout` component (NOT spawnable alone); set via `TextLayout::new_with_justify(Justify::Center)` / `TextLayout::default().with_justify(..)`
- `Text2d`/`Text2dShadow` -> `bevy::sprite::Text2d` — world-space text in `bevy_sprite`
- `TextFont { line_height }` -> `LineHeight` separate component (required by `Text`/`Text2d`/`TextSpan`)
- `BreakLineOn` -> `LineBreak`; `TextFont::from_font(h)` -> `TextFont::from(h)`
- `TextLayoutInfo::size` -> `.logical_size`; `.section_rects` -> `.run_geometry: Vec<RunGeometry>`

### Color
sRGB is explicit; no channel accessors; CSS constants moved.
```rust
let c = Color::srgb(1.0, 0.0, 0.0);
use bevy::color::palettes::css::BLUE;
let hsl: Hsla = Srgba::hex("#ffffff").unwrap().into();
```
- `Color::rgb`/`rgba` -> `Color::srgb`/`srgba`; `Color::rgb_linear` -> `Color::linear_rgb`
- `Color::Rgba { .. }` -> `Color::Srgba(Srgba { .. })` — each space its own struct
- `Color::hex(..)` -> `Srgba::hex(..)`; `Color::BLUE` -> `bevy::color::palettes::css::BLUE`
- `color.r()`/`set_r()` -> convert to `Srgba`/`Hsla` then mutate field — accessors removed
- `color.a()`/`set_a()`/`with_a()` -> `.alpha()`/`.set_alpha()`/`.with_alpha()`
- `Color::linear()` -> `Color::to_linear()`; `color.as_hsla()` -> `let c: Hsla = c.into()`
- `Color * f32` -> use `LinearRgba { .. } * f32` — alpha now scaled too

### Picking
```rust
commands.spawn((Node::default(), Pickable::default()))
    .observe(|ev: On<Pointer<Press>>| { /* ... */ });
```
- `PickingBehavior` -> `Pickable`
- `Pointer<Down>`/`Pointer<Up>` -> `Pointer<Press>`/`Pointer<Release>` — (`Pressed` is now a held-state marker)
- `PointerAction::Pressed` -> `PointerAction::Press` / `PointerAction::Release`
- `PointerBundle::new(PointerId::Mouse)` -> `PointerId::Mouse` (component directly)
- `Pointer.target` -> `On::original_event_target()` (observers)
- `PickingPlugin`/`PointerInputPlugin` fields -> `PickingSettings`/`PointerInputSettings` resources
- `bevy_picking::Location` (component) -> `PointerLocation` (wraps `Location`)

## Assets, Scene, Animation, Audio & Gizmos

### Audio
Audio is component-based (`AudioPlayer`), never bundles.
```rust
commands.spawn((AudioPlayer::new(asset_server.load("sfx.ogg")), PlaybackSettings::LOOP));
```
- `audio.play_with_settings(handle, settings)` / `AudioBundle { source, settings }` -> spawn `(AudioPlayer::new(handle), PlaybackSettings)` — bundle removed
- `Volume::new_relative(x)` / `Volume::new_absolute(x)` / `Volume::new(x)` / `Volume(x)` -> `Volume::Linear(x)` — enum variant required (`Volume::Decibels` also)
- `Volume::ZERO` -> `Volume::SILENT` — renamed
- `SpatialSettings::new(...)` -> `PlaybackSettings::with_spatial()` — plus `SpatialListener` component
- `AudioSinkPlayback::toggle()` -> `AudioSinkPlayback::toggle_playback()` — vs `toggle_mute`
- `set_volume(&self)` -> `set_volume(&mut self)` — needs `&mut`
- `vol_a + vol_b` -> `volume.increase_by_percentage(pct)` — `Add`/`Sub` impls removed
- `PlaybackMode::Despawn` despawns children too — use `Remove` to keep them
- `AudioPlugin { spatial_scale }` -> `AudioPlugin { default_spatial_scale }` — renamed

### Scene
- `commands.spawn_scene(asset)` / `SceneBundle { scene, .. }` -> spawn `SceneRoot(asset)` (or `DynamicSceneRoot` for dynamic) — required-component, spawns as child
- `DynamicScene::from_world(&world, registry)` -> `DynamicScene::from_world(&world)` — registry arg removed
- `scene.serialize_ron()` -> `scene.serialize(&type_registry)` — needs `&TypeRegistry`
- `SceneSpawner::despawn(dynamic)` -> `SceneSpawner::despawn_dynamic` — bare names now act on `Scene`
- `ron` re-export from `bevy_scene`/`bevy_asset` removed — add `ron` as direct dep

### Animation
`AnimationPlayer` requires an `AnimationGraphHandle`; bones need two components.
```rust
let (graph, idx) = AnimationGraph::from_clip(clips.add(clip));
commands.spawn((AnimationPlayer::default(), AnimationGraphHandle(graphs.add(graph))));
// on each bone entity:
commands.entity(bone).insert((AnimationTargetId(id), AnimatedBy(player_entity)));
```
- `player.play(animations.add(animation))` -> `player.play(node_index)` with `AnimationGraph` — graph required
- `Handle<AnimationGraph>` as component -> `AnimationGraphHandle(handle)` — wrapper required
- `AnimationTarget { id, player }` -> `(AnimationTargetId(id), AnimatedBy(player))` — split into two components
- `play_with_transition()` -> `AnimationTransitions` component — method removed
- `elapsed()` / `set_elapsed()` -> `seek_time()` / `seek_to()` — renamed
- `stop_repeating()` -> `set_repeat(RepeatAnimation::Never)` — unified
- `is_playing_animation()` -> `animation_is_playing()` — renamed
- `#[derive(Event)]` anim event -> `#[derive(AnimationEvent)]`; handler `|m: On<SayMessage>|`, field via `m.trigger().target` — was `animation_player`
- `EaseFunction::Steps(n)` -> `EaseFunction::Steps(n, JumpAt::default())` — new param

### Assets — handles & IDs
- `HandleUntyped` -> `UntypedHandle`; `HandleId` -> `AssetId<T>` / `UntypedAssetId`
- `handle.id` (field) / `let id: AssetId<T> = handle.into()` -> `handle.id()` — accessor; `Into` removed
- `handle.clone_weak()` -> `handle.clone()` — removed
- `Handle::weak_from_u128(n)` / `weak_handle!("uuid")` -> `uuid_handle!("uuid")` — `Handle::Uuid`
- `AssetPath::new("a.png", None)` -> `AssetPath::from("a.png")`; label via `.with_label("Mesh0")`
- `asset_server.load("foo.glb")` -> `asset_server.load("foo.glb#Scene0")` — gLTF sub-asset label required

### Assets — events, state, config
- `AssetEvent::Added { handle }` -> `AssetEvent::Added { id }` — carries `AssetId`; also `LoadedWithDependencies { id }`
- `LoadState::Failed` -> `LoadState::Failed(AssetLoadError)`; compare via `matches!`/`state.is_loaded()` — no `PartialEq`/`Copy`
- `.insert_resource(AssetMetaCheck::Never)` -> `AssetPlugin { meta_check: AssetMetaCheck::Never, ..default() }` — plugin field
- `AssetPlugin { watch_for_changes: true }` -> `AssetPlugin { watch_for_changes_override: Some(true), ..default() }`; enable `file_watcher` feature
- `add_systems(UpdateAssets, sys)` -> `add_systems(PreUpdate, sys)`; `AssetEvents` schedule -> `First` + `.in_set(AssetEvents)`
- `assets.insert(id, asset)` -> `assets.insert(id, asset).unwrap()` — now returns `Result`

### Assets — module moves & types
- `TextureAtlas` (combined) -> `TextureAtlasLayout` + `Handle<Image>` — split layout/image
- `bevy_render::{Image, ImagePlugin, ImageFormat, ImageSampler, TextureAtlas}` -> `bevy::image::*` — moved
- `bevy_render::RenderAssetUsages` -> `bevy::asset::RenderAssetUsages` — moved
- `GltfLoaderSettings::load_meshes: bool` -> `RenderAssetUsages` — type changed
- `GltfPlugin { use_model_forward_direction }` -> `GltfPlugin { convert_coordinates: GltfConvertCoordinates { rotate_scene_entity, rotate_meshes } }`

### Assets — custom loaders (footguns)
- `fn load(..) -> BoxedFuture<...>` -> `async fn load(...) -> Result<...>` — async fn, not object-safe
- `impl AssetLoader for L<'a>` -> `impl AssetLoader for L` — drop named lifetimes
- `type Asset` + return value replaces `set_default_asset`; loaders need `TypePath` super trait (`#[derive(TypePath)]`)
- `load_context.asset_path()` -> `load_context.path()` returns `AssetPath`; `.path().path()` for `&Path`
- `load_context.load_direct(p)` -> `load_context.loader().immediate().load(p)` — builder; `.with_unknown_type()`/`.with_static_type()`
- `AssetSourceBuilder::build().with_reader(r)` -> `AssetSourceBuilder::new(reader_fn)` — reader mandatory

### Gizmos
- `Gizmos::cuboid(...)` -> `Gizmos::cube(...)` — renamed
- `GizmoConfig` (resource) -> `GizmoConfigStore` + `DefaultGizmoConfigGroup` — config store
- `gizmos.circle(pos, radius, normal: Vec3)` -> `normal: Dir3` — direction type
- `gizmos.segments()` -> `gizmos.resolution()` (u32) — renamed
- gizmo methods with separate translation/rotation -> single `Isometry3d`/`Isometry2d` arg
- `gizmos.primitive_2d(poly, ..)` -> `gizmos.primitive_2d(&poly, ..)` — by reference; methods return builders
- `App::insert_gizmo_group()` -> `App::insert_gizmo_config()` — renamed

## Input, Time, Window, Transform, Math, Reflection & Misc

### Input
- `Input<T>` -> `ButtonInput<T>` — resource renamed; `Res<ButtonInput<KeyCode>>`
- `ElementState` -> `ButtonState` — keyboard/mouse press state enum
- `Interaction::Clicked` -> `Interaction::Pressed`
- `KeyCode::W` -> `KeyCode::KeyW` — letters use `Key` prefix (physical keys)
- `KeyCode::Up` -> `KeyCode::ArrowUp` — arrows use `Arrow` prefix
- `KeyCode::Key1` -> `KeyCode::Digit1` — number row uses `Digit`
- `KeyCode::LShift` -> `KeyCode::ShiftLeft` — also `ControlLeft`/`AltLeft`/`SuperLeft`, `*Right`
- `KeyCode::LBracket` -> `KeyCode::BracketLeft`
- `ReceivedCharacter` -> REMOVED — match `KeyboardInput.logical_key` for `Key::Character`
- `Gamepads` resource / `Input::update()` -> `Query<&Gamepad>` (entities) / `ButtonInput::clear()`
- gamepad button/axis state -> read from the `Gamepad` component per entity
- `bevy::input::touchpad` `TouchpadMagnify`/`TouchpadRotate` -> `bevy::input::gestures` `PinchGesture`/`RotationGesture`
- `bevy::a11y::Focus` -> `bevy::input_focus::InputFocus`

Input sources are feature-gated in 0.18: enable `mouse`, `keyboard`, `gamepad`, `touch`, `gestures`.

### Time
- `time.time_since_startup()` -> `time.elapsed()` — returns `Duration`
- `time.elapsed_seconds()` -> `time.elapsed_secs()` / `elapsed_secs_f64()`
- `time.delta_seconds()` -> `time.delta_secs()`; `timer.tick()` takes a `Duration`
- `Res<FixedTime>` / `FixedTime.period` -> `Res<Time<Fixed>>` / `Time<Fixed>::delta()`
- `FixedTime::new_from_secs(s)` -> `Time::<Fixed>::from_seconds(s)`
- `Res<Time>` raw clock -> `Res<Time<Real>>`; pausing -> `ResMut<Time<Virtual>>::pause()`
- `Timer::new(d, false)` -> `Timer::new(d, TimerMode::Once)` (`true` -> `TimerMode::Repeating`)
- `Timer::percent()` -> `Timer::fraction()`; `percent_left()` -> `fraction_remaining()`
- `Time::<Fixed>::overstep_percentage()` -> `overstep_fraction()`
- `timer.paused()` / `timer.finished()` -> `timer.is_paused()` / `timer.is_finished()`
- import `Timer`/`Time` from `bevy::time::*` (not `bevy::core`)

### Window
Windows are entities (`Window` component); in 0.18 `Window` is split into multiple components.
```rust
Query<&Window, With<PrimaryWindow>>
Single<&mut CursorOptions, With<PrimaryWindow>> // cursor_options.grab_mode
```
- `Res<Windows>` / `windows.get_primary()` -> `Query<&Window, With<PrimaryWindow>>`
- `WindowDescriptor` -> `Window` component; `WindowPlugin { primary_window: Some(Window {..}) }`
- width/height fields -> `WindowResolution::new(1920, 1080)` — `u32` args (0.17+)
- `vsync: false` -> `present_mode: PresentMode::Immediate`
- `cursor_locked: bool` -> `cursor_options.grab_mode: CursorGrabMode`
- cursor options on `Window` -> `WindowPlugin { primary_cursor_options: Some(CursorOptions {..}) }`
- `Window::always_on_top` -> `Window::window_level: WindowLevel::AlwaysOnTop`
- `close_on_esc` / `exit_on_esc_system` -> REMOVED — roll your own with `ButtonInput<KeyCode>` + `Escape`
- `UpdateMode::Reactive { max_wait }` -> `UpdateMode::Reactive { wait }`
- `CustomCursor::Image(..)`/`Url(..)` -> `CustomCursorImage`/`CustomCursorUrl` structs
- `WinitPlugin<M>` generic -> `WinitPlugin`; `send_event(WinitUserEvent::WakeUp)`
- cursor types now in `bevy::window` (moved from `bevy_winit`)

### Transform
- `Transform::identity()` -> `Transform::IDENTITY` (associated const)
- `transform.mul_vec3(v)` -> `transform.transform_point(v)`
- `global_transform.translation` field -> `global_transform.translation()` method
- `Transform::forward()`/`up()`/`right()` now return `Dir3` (deref for `Vec3`); same on `GlobalTransform`
- `transform.rotate_axis(Vec3::X, a)` -> `transform.rotate_axis(Dir3::X, a)`
- `GlobalTransform::compute_matrix()` -> `to_matrix()` (also `Transform::to_matrix()`)
- `SpatialBundle::VISIBLE_IDENTITY` -> `INHERITED_IDENTITY`
- manually removing `Aabb` to refresh -> auto-updates in 0.18; add `NoAutoAabb` to opt out

### Math
- `Vec3::as_i32()` -> `Vec3::as_ivec3()` (and `Vec2::as_ivec2()`)
- `Ray` -> `Ray3d` (+ `Ray2d`); `Ray3d::new(pos, Dir3)` takes a typed direction
- `Direction3d`/`Direction2d` -> `Dir3`/`Dir2`; `from_normalized(v)` -> `Dir3::new_unchecked(v)`
- `Capsule` -> `Capsule3d`/`Capsule2d`
- `Bezier` -> `CubicBezier`; `BSpline`/`Hermite` -> `CubicBSpline`/`CubicHermite`
- `Plane3d::new(Vec3::Y)` (infinite) -> `InfinitePlane3d::new(Vec3::Y)`; `Plane3d` is finite with `half_size`
- `Rot2::angle_between()` -> `Rot2::angle_to()`
- `impl Point` -> `impl VectorSpace`; `FloatOrd` lives in `bevy::math`

### Reflection
- `#[derive(Reflect, FromReflect)]` -> `#[derive(Reflect)]` — `FromReflect` auto-derived
- `#[derive(TypeUuid)]` -> `#[derive(TypePath)]`; `std::any::type_name` -> `TypePath::type_path()`
- `Reflect::get_type_info()` -> `Reflect::represented_type_info()`
- `#[reflect[Clone]]` / `#[reflect{Clone}]` -> `#[reflect(Clone)]` — parentheses only (0.18)
- `UntypedReflectDeserializer` -> `ReflectDeserializer`
- `ChangeTrackers<T>` -> `Ref<T>`; `handle.id` field -> `handle.id()`
- types auto-registered in 0.18 (`reflect_auto_register`); drop most `register_type::<T>()` (generics still manual)

### Misc
- `PrintDiagnosticsPlugin` -> `LogDiagnosticsPlugin`; `Res<Diagnostics>` -> `Res<DiagnosticsStore>` (write side is `Diagnostics` SystemParam)
- `ComputeTaskPool::init()` -> `get_or_init()` (also `AsyncCompute`/`Io`)
- feature `multi-threaded` -> `multi_threaded`; `bevy::utils` HashMap etc. -> `bevy_platform`
- `HashMap::get_many_mut` -> `get_disjoint_mut`
