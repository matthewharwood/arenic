//! **Author mode** (`just author` — the `author` cargo feature; never in a
//! shipping build). The in-game tooling for choreographing LAYERED encounters
//! (`_docs/AUTHORING_UI.md`):
//!
//! - **`B`** possesses / releases the current arena's [`Boss`]: `Selected`
//!   moves onto the boss root, and from there the ENTIRE hero flow applies
//!   unchanged — arrows step it, `1`-`4` cast its phase-loadout slots
//!   ([`boss_cast_slots`] live-casts exactly what playback will resolve),
//!   `R` records its 2-minute staff, Commit folds it back in as a ghost AND
//!   stages the take into the arena's DRAFT [`ArenaStack`] boss layer.
//! - **`W`** PUBLISHES the focused arena's whole draft stack as the next
//!   versioned `assets/encounters/<arena>/<difficulty>/layers.vNNNN.ron` —
//!   the atomic unit the game folds at every cycle wrap.
//! - **`D`** cycles the [`ActiveDifficulty`] — each difficulty owns its own
//!   stack; a dirty draft blocks the swap until published.
//! - **`T`** opens the tile editor on the current arena (its clock pauses and
//!   the world keys yield): arrows steer the cell cursor, **`,` / `.`** scrub
//!   the timeline ±1s (Shift: ±10s) with ghosts re-derived at the scrubbed
//!   tick, **`I` / `O`** set the in/out marks, **Space** toggles a lava
//!   keyframe on the cursor's cell over `[in, out)` in the SELECTED tile
//!   layer (**`Tab`** cycles tile layers, **`N`** adds one on top — the top
//!   layer wins overlapping cells).
//! - **`F5`** restarts the current arena for a fresh watch-through.
//!
//! Rolling back is filesystem-native: delete the newest `layers.vNNNN.ron`
//! and the game re-reads the survivor at the next cycle wrap (the files are
//! committed to git, so history is the second level of undo).

use arenic_game::Boss;
use arenic_game::ability::{AbilityMeshes, AbilitySfx, cast};
use arenic_game::arena::Arena;
use arenic_game::audio::AudioMix;
use arenic_game::default_font::MonoFont;
use arenic_game::encounter::{ActiveDifficulty, encounter_dir, resolve_ability};
use arenic_game::grid::{MAX_COL, MAX_ROW, TileMover, arrow_delta, arrow_pressed, tile_to_world};
use arenic_game::layer::{ArenaStack, Layer, LayerId, LayerKind, write_stack};
use arenic_game::tile::TileKind;
use arenic_game::tile_script::{TileKeyframe, TileScript, TileSelector};
use arenic_game::timeline::{
    Action, ArenaClock, ArenaTimeline, CYCLE_TICKS, Ghost, TICKS_PER_SECOND, TimelineEvent,
    restart, seek_window,
};
use bevy::audio::DefaultSpatialScale;
use bevy::input::common_conditions::input_just_pressed;
use bevy::light::{NotShadowCaster, NotShadowReceiver};
use bevy::prelude::*;

use crate::hud::TOP_BAR_PX;
use crate::intro_scene::{CurrentArena, Puck, Selected};
use crate::modal::no_modal;
use crate::recording::{
    DraftTimeline, RecordingCommitted, RecordingState, fmt_tick, is_idle, no_pending_walk,
    not_counting_down, snap_arena_ghosts,
};
use crate::score_sync::LoadedScores;
use crate::states::{AppState, TileEditMode, not_tile_editing};

/// The tile editor's working state — persists across `T` toggles so marks and
/// the cursor stay where you left them. Whether the editor is OPEN is the
/// [`TileEditMode`] resource (the one flag the input gates watch).
#[derive(Resource)]
struct TileEditor {
    /// The cell the cursor sits on.
    cursor: IVec2,
    /// New keyframes span `[mark_in, mark_out)` ticks.
    mark_in: u32,
    mark_out: u32,
    /// The tile LAYER paints land in (`Tab` cycles, `N` creates). `None`
    /// falls back to the topmost tile layer, creating "Tiles 1" on first
    /// paint.
    selected_layer: Option<LayerId>,
}

impl Default for TileEditor {
    fn default() -> Self {
        Self {
            cursor: IVec2::new(33, 15),
            mark_in: 0,
            mark_out: CYCLE_TICKS,
            selected_layer: None,
        }
    }
}

/// The glowing cell-cursor ring the tile editor steers.
#[derive(Component)]
#[component(immutable)]
struct TileCursor;

/// The author HUD chip's text node.
#[derive(Component)]
#[component(immutable)]
struct AuthorHud;

/// `true` while the tile editor is open (the author-side counterpart of
/// [`not_tile_editing`]).
fn tile_editing(mode: Option<Res<TileEditMode>>) -> bool {
    mode.is_some()
}

/// `true` when any arena's stack changed this frame — re-renders the HUD chip
/// after commits, paints, publishes, and sync swaps.
fn stacks_changed(stacks: Query<(), Changed<ArenaStack>>) -> bool {
    !stacks.is_empty()
}

/// `B` — possess the current arena's boss, or release a possessed one back to
/// puck 0 (focus follows the selection both ways, like Tab). The Guild House
/// has no boss; there B does nothing.
fn possess_boss(
    mut commands: Commands,
    mut current: ResMut<CurrentArena>,
    selected: Single<(Entity, Has<Boss>), With<Selected>>,
    bosses: Query<(Entity, &ChildOf), With<Boss>>,
    pucks: Query<(Entity, &Puck, &ChildOf)>,
    arenas: Query<&Arena>,
) -> Result {
    let (holder, is_boss) = *selected;
    if is_boss {
        let Some((puck, _, child_of)) = pucks.iter().min_by_key(|(_, puck, _)| puck.0) else {
            return Ok(());
        };
        commands.entity(holder).remove::<Selected>();
        commands.entity(puck).insert(Selected);
        current.0 = arenas.get(child_of.parent())?.index();
    } else if let Some((boss, _)) = bosses.iter().find(|(_, child_of)| {
        arenas
            .get(child_of.parent())
            .is_ok_and(|arena| arena.index() == current.0)
    }) {
        commands.entity(holder).remove::<Selected>();
        commands.entity(boss).insert(Selected);
    }
    Ok(())
}

/// `D` — cycle the authoring difficulty. `score_sync` notices the change and
/// re-points every arena at that difficulty's stack — which would clobber
/// unsaved layer edits, so a dirty draft blocks the swap until it's published
/// (`W`).
fn cycle_difficulty(
    mut difficulty: ResMut<ActiveDifficulty>,
    stacks: Query<(&Arena, &ArenaStack)>,
) {
    if let Some((arena, _)) = stacks.iter().find(|(_, stack)| stack.dirty()) {
        warn!(
            "{}: unsaved layer edits — press W to publish before switching difficulty",
            arena.name()
        );
        return;
    }
    difficulty.0 = difficulty.0.next();
    info!("authoring difficulty: {:?}", difficulty.0);
}

/// `1`-`4` — live-cast a possessed boss's loadout slots, resolved exactly as
/// playback will resolve them ([`resolve_ability`] at the current tick), and
/// capture the draft event atomically with the cast — so the rehearsed take
/// can never disagree with the committed staff. The boss never goes through
/// the hero paths (`fire_holy_nova` excludes it; `capture_intent` skips it):
/// hero slot 1 is hard-wired Holy Nova, but a boss's slot 1 follows whatever
/// its phase loadout says.
fn boss_cast_slots(
    mut commands: Commands,
    keys: Res<ButtonInput<KeyCode>>,
    difficulty: Res<ActiveDifficulty>,
    state: Res<RecordingState>,
    mut draft: ResMut<DraftTimeline>,
    boss: Single<(Entity, &ChildOf), (With<Selected>, With<Boss>, Without<Ghost>)>,
    arenas: Query<(&Arena, &ArenaClock)>,
    meshes: Res<AbilityMeshes>,
    mut materials: ResMut<Assets<StandardMaterial>>,
    sfx: Res<AbilitySfx>,
    mix: Res<AudioMix>,
    scale: Res<DefaultSpatialScale>,
) -> Result {
    let (boss, child_of) = *boss;
    let (arena, clock) = arenas.get(child_of.parent())?;
    const SLOTS: [(KeyCode, KeyCode, u8); 4] = [
        (KeyCode::Digit1, KeyCode::Numpad1, 1),
        (KeyCode::Digit2, KeyCode::Numpad2, 2),
        (KeyCode::Digit3, KeyCode::Numpad3, 3),
        (KeyCode::Digit4, KeyCode::Numpad4, 4),
    ];
    // The press-direction AIM stub (ARE-46): arrows HELD at the moment of a
    // slot press become the cast's tile-space direction — the keyboard
    // stand-in for the future press-drag-release mouse gesture. Radial
    // abilities ignore it; the payload round-trips today.
    let held = IVec2::new(
        (keys.pressed(KeyCode::ArrowRight) as i32)
            .strict_sub(keys.pressed(KeyCode::ArrowLeft) as i32),
        (keys.pressed(KeyCode::ArrowUp) as i32).strict_sub(keys.pressed(KeyCode::ArrowDown) as i32),
    );
    let aim = (held != IVec2::ZERO).then_some(held);
    for (digit, numpad, slot) in SLOTS {
        if !(keys.just_pressed(digit) || keys.just_pressed(numpad)) {
            continue;
        }
        if let Some(id) = resolve_ability(true, *arena, difficulty.0, slot, clock.tick) {
            cast(
                &mut commands,
                id,
                &meshes,
                &mut materials,
                &sfx,
                &mix,
                &scale,
                aim,
                boss,
            );
        }
        if matches!(*state, RecordingState::Recording) {
            draft.events.push(TimelineEvent {
                tick: clock.tick,
                action: Action::Ability { slot, aim },
            });
        }
    }
    Ok(())
}

/// Stages a committed BOSS staff into its arena's DRAFT stack (hero commits
/// stay session-local, exactly as in the plain game). Nothing touches disk
/// here — the world already previews the take (the commit folded it live);
/// `W` publishes the whole stack.
fn commit_to_layer(
    mut commands: Commands,
    mut commits: MessageReader<RecordingCommitted>,
    difficulty: Res<ActiveDifficulty>,
    bosses: Query<&ChildOf, With<Boss>>,
    mut stacks: Query<&mut ArenaStack>,
) {
    for commit in commits.read() {
        let Ok(child_of) = bosses.get(commit.entity) else {
            continue;
        };
        if commit.difficulty != difficulty.0 {
            // Stale commit from before a difficulty swap — never stage it
            // into another difficulty's draft.
            warn!(
                "{}: dropping a {:?} take staged after swapping to {:?}",
                commit.arena.name(),
                commit.difficulty,
                difficulty.0
            );
            continue;
        }
        let arena_entity = child_of.parent();
        if let Ok(mut arena_stack) = stacks.get_mut(arena_entity) {
            let boss_layer = arena_stack
                .stack
                .layers
                .iter_mut()
                .find(|layer| matches!(layer.kind, LayerKind::Boss(_)));
            match boss_layer {
                Some(layer) => layer.kind = LayerKind::Boss(commit.recording.clone()),
                None => {
                    let id = arena_stack.stack.next_id();
                    arena_stack.stack.layers.push(Layer::new(
                        id,
                        "Boss",
                        LayerKind::Boss(commit.recording.clone()),
                    ));
                }
            }
        } else {
            // First take on a never-published arena: a fresh draft stack.
            let mut arena_stack = ArenaStack::default();
            arena_stack.stack.layers.push(Layer::new(
                LayerId(0),
                "Boss",
                LayerKind::Boss(commit.recording.clone()),
            ));
            commands.entity(arena_entity).insert(arena_stack);
        }
        info!(
            "{}: boss take staged in the draft — W publishes the stack",
            commit.arena.name()
        );
    }
}

/// `T` — toggle the tile editor on the current arena: pause/release its clock,
/// show/hide the cell cursor (spawned on first use), and raise/drop the
/// [`TileEditMode`] flag the world-input gates watch.
fn toggle_tile_mode(
    mut commands: Commands,
    editing: Option<Res<TileEditMode>>,
    editor: Res<TileEditor>,
    current: Res<CurrentArena>,
    mut arenas: Query<(Entity, &Arena, &mut ArenaClock)>,
    cursor: Option<Single<(Entity, &mut Visibility), With<TileCursor>>>,
    mut meshes: ResMut<Assets<Mesh>>,
    mut materials: ResMut<Assets<StandardMaterial>>,
) {
    let Some((arena_entity, arena, mut clock)) = arenas
        .iter_mut()
        .find(|(_, arena, _)| arena.index() == current.0)
    else {
        return;
    };
    if editing.is_some() {
        commands.remove_resource::<TileEditMode>();
        clock.paused = false;
        if let Some(cursor) = cursor {
            let (_, mut visibility) = cursor.into_inner();
            *visibility = Visibility::Hidden;
        }
        return;
    }
    commands.insert_resource(TileEditMode);
    clock.paused = true;
    let p = tile_to_world(editor.cursor.x, editor.cursor.y);
    match cursor {
        Some(cursor) => {
            // Re-show the existing ring on (possibly) a different arena.
            let (entity, mut visibility) = cursor.into_inner();
            *visibility = Visibility::Inherited;
            commands
                .entity(entity)
                .insert((ChildOf(arena_entity), Transform::from_xyz(p.x, p.y, 0.04)));
        }
        None => {
            let warning = arena.theme().palette().warning;
            let l = warning.to_linear();
            commands.spawn((
                TileCursor,
                DespawnOnExit(AppState::Intro),
                Mesh3d(meshes.add(Annulus::new(0.09, 0.125))),
                MeshMaterial3d(materials.add(StandardMaterial {
                    base_color: warning,
                    emissive: LinearRgba::rgb(l.red, l.green, l.blue) * 3.0,
                    ..default()
                })),
                Transform::from_xyz(p.x, p.y, 0.04),
                NotShadowCaster,
                NotShadowReceiver,
                ChildOf(arena_entity),
            ));
        }
    }
}

/// Arrows — steer the tile cursor one cell per press (the editor owns the
/// arrows while open; `move_selected` is gated off).
fn move_cursor(
    keys: Res<ButtonInput<KeyCode>>,
    mut editor: ResMut<TileEditor>,
    cursor: Single<&mut Transform, With<TileCursor>>,
) {
    let delta = arrow_delta(&keys);
    editor.cursor.x = editor.cursor.x.strict_add(delta.x).clamp(0, MAX_COL);
    editor.cursor.y = editor.cursor.y.strict_add(delta.y).clamp(0, MAX_ROW);
    let p = tile_to_world(editor.cursor.x, editor.cursor.y);
    let mut transform = cursor.into_inner();
    transform.translation.x = p.x;
    transform.translation.y = p.y;
}

/// `,` / `.` — scrub the paused arena ±1 second (Shift: ±10s). Ghost poses are
/// re-derived deterministically: snap every ghost to its start, re-apply the
/// `Move` events up to the target tick ([`seek_window`]), and park the clock
/// there. Tile previews follow on the next fixed tick via `play_tile_scripts`.
fn scrub(
    keys: Res<ButtonInput<KeyCode>>,
    current: Res<CurrentArena>,
    mut arenas: Query<(Entity, &Arena, &mut ArenaClock, &mut ArenaTimeline)>,
    mut ghosts: Query<(Entity, &Ghost, &ChildOf, &mut TileMover, &mut Transform)>,
) {
    let dir = (keys.just_pressed(KeyCode::Period) as i64)
        .strict_sub(keys.just_pressed(KeyCode::Comma) as i64);
    if dir == 0 {
        return;
    }
    let shift = keys.pressed(KeyCode::ShiftLeft) || keys.pressed(KeyCode::ShiftRight);
    let step = (TICKS_PER_SECOND as i64).strict_mul(if shift { 10 } else { 1 });
    let Some((arena_entity, _, mut clock, mut timeline)) = arenas
        .iter_mut()
        .find(|(_, arena, ..)| arena.index() == current.0)
    else {
        return;
    };
    let target = (clock.tick as i64)
        .strict_add(dir.strict_mul(step))
        .clamp(0, CYCLE_TICKS.strict_sub(1) as i64) as u32;
    snap_arena_ghosts(arena_entity, &mut ghosts, None);
    let (window, cursor) = seek_window(&timeline.events, target);
    for event in window {
        if let Action::Move(delta) = event.action
            && let Ok((.., mut mover, mut transform)) = ghosts.get_mut(event.ghost)
        {
            mover.step(&mut transform, delta);
        }
    }
    timeline.cursor = cursor;
    clock.tick = target;
}

/// `I` / `O` — set the in/out marks (the tick range newly painted keyframes
/// get) to the scrubbed clock. `O` lands on the *end* of the current tick so a
/// `[I, O]` gesture always covers what you watched, and the pair is kept
/// non-empty.
fn set_marks(
    keys: Res<ButtonInput<KeyCode>>,
    mut editor: ResMut<TileEditor>,
    current: Res<CurrentArena>,
    arenas: Query<(&Arena, &ArenaClock)>,
) {
    let Some((_, clock)) = arenas.iter().find(|(arena, _)| arena.index() == current.0) else {
        return;
    };
    if keys.just_pressed(KeyCode::KeyI) {
        editor.mark_in = clock.tick;
        if editor.mark_out <= editor.mark_in {
            editor.mark_out = CYCLE_TICKS;
        }
    }
    if keys.just_pressed(KeyCode::KeyO) {
        editor.mark_out = clock.tick.strict_add(1);
        if editor.mark_in >= editor.mark_out {
            editor.mark_in = editor.mark_out.strict_sub(1);
        }
    }
}

/// `Space` — toggle a Lava keyframe on the cursor's cell over `[in, out)` in
/// the arena's live [`TileScript`] (created on first paint). Repainting a cell
/// removes its existing keyframe(s) instead, whatever range they had — the
/// board preview updates on the next fixed tick either way.
fn paint(
    mut commands: Commands,
    mut editor: ResMut<TileEditor>,
    current: Res<CurrentArena>,
    mut arenas: Query<(
        Entity,
        &Arena,
        Option<&mut ArenaStack>,
        Option<&mut TileScript>,
    )>,
) {
    let Some((arena_entity, _, arena_stack, script)) = arenas
        .iter_mut()
        .find(|(_, arena, ..)| arena.index() == current.0)
    else {
        return;
    };
    let selector = TileSelector::Cell {
        col: editor.cursor.x as u8,
        row: editor.cursor.y as u8,
    };
    let keyframe = TileKeyframe {
        from: editor.mark_in,
        to: editor.mark_out,
        selector,
        kind: TileKind::Lava,
    };

    // Paint lands in the SELECTED tile layer of the DRAFT stack — falling
    // back to the topmost tile layer, creating "Tiles 1" on first paint.
    let created = arena_stack.is_none();
    let mut fresh_stack;
    let stack = match arena_stack {
        Some(stack) => stack.into_inner(),
        None => {
            fresh_stack = ArenaStack::default();
            &mut fresh_stack
        }
    };
    let target = editor
        .selected_layer
        .filter(|&id| {
            stack
                .stack
                .layer(id)
                .is_some_and(|layer| matches!(layer.kind, LayerKind::Tiles(_)))
        })
        .or_else(|| {
            stack
                .stack
                .layers
                .iter()
                .rev()
                .find(|layer| matches!(layer.kind, LayerKind::Tiles(_)))
                .map(|layer| layer.id)
        });
    let target = match target {
        Some(id) => id,
        None => {
            let id = stack.stack.next_id();
            stack
                .stack
                .layers
                .push(Layer::new(id, "Tiles 1", LayerKind::Tiles(Vec::new())));
            id
        }
    };
    editor.selected_layer = Some(target);
    if let Some(layer) = stack.stack.layer_mut(target)
        && let LayerKind::Tiles(keyframes) = &mut layer.kind
    {
        let before = keyframes.len();
        keyframes.retain(|k| k.selector != selector);
        if keyframes.len() == before {
            keyframes.push(keyframe);
        }
    }

    // Regenerate the live evaluation cache from the whole stack.
    let merged = stack.stack.merged_tiles();
    match script {
        Some(mut script) => script.keyframes = merged,
        None => {
            commands.entity(arena_entity).insert(TileScript {
                keyframes: merged,
                applied: default(),
            });
        }
    }
    // A never-stacked arena gets its fresh draft attached.
    if created {
        commands.entity(arena_entity).insert(stack.clone());
    }
}

/// `W` — PUBLISH the focused arena's whole draft stack as the next versioned
/// `layers.vNNNN.ron`, re-baseline the draft, and bump the ledger so the
/// wrap-check doesn't re-fold what's already playing.
fn publish_stack(
    current: Res<CurrentArena>,
    difficulty: Res<ActiveDifficulty>,
    mut loaded: ResMut<LoadedScores>,
    mut arenas: Query<(&Arena, &mut ArenaStack)>,
) {
    let Some((arena, mut stack)) = arenas
        .iter_mut()
        .find(|(arena, _)| arena.index() == current.0)
    else {
        return;
    };
    match write_stack(
        &encounter_dir(*arena, difficulty.0),
        *arena,
        difficulty.0,
        &stack.stack,
    ) {
        Ok((version, path)) => {
            stack.mark_published(version);
            loaded
                .0
                .insert(arena.index() as u8, Some((difficulty.0, version)));
            info!(
                "{}: stack v{version} published ({})",
                arena.name(),
                path.display()
            );
        }
        Err(err) => warn!("{}: stack publish FAILED: {err}", arena.name()),
    }
}

/// Tab (tile mode) — cycle the paint-target among the stack's tile layers.
fn cycle_tile_layer(
    mut editor: ResMut<TileEditor>,
    current: Res<CurrentArena>,
    arenas: Query<(&Arena, &ArenaStack)>,
) {
    let Some((arena, stack)) = arenas.iter().find(|(arena, _)| arena.index() == current.0) else {
        return;
    };
    let tiles: Vec<LayerId> = stack
        .stack
        .layers
        .iter()
        .filter(|layer| matches!(layer.kind, LayerKind::Tiles(_)))
        .map(|layer| layer.id)
        .collect();
    if tiles.is_empty() {
        return;
    }
    let next = match editor
        .selected_layer
        .and_then(|id| tiles.iter().position(|&t| t == id))
    {
        Some(at) => tiles[at.strict_add(1) % tiles.len()],
        None => tiles[0],
    };
    editor.selected_layer = Some(next);
    let name = stack
        .stack
        .layer(next)
        .map(|layer| layer.name.as_str())
        .unwrap_or("?");
    info!("{}: painting into layer \"{name}\"", arena.name());
}

/// `N` (tile mode) — add a fresh tile layer on TOP of the stack (it wins
/// conflicts) and select it for painting.
fn new_tile_layer(
    mut commands: Commands,
    mut editor: ResMut<TileEditor>,
    current: Res<CurrentArena>,
    mut arenas: Query<(Entity, &Arena, Option<&mut ArenaStack>)>,
) {
    let Some((arena_entity, arena, arena_stack)) = arenas
        .iter_mut()
        .find(|(_, arena, _)| arena.index() == current.0)
    else {
        return;
    };
    let created = arena_stack.is_none();
    let mut fresh_stack;
    let stack = match arena_stack {
        Some(stack) => stack.into_inner(),
        None => {
            fresh_stack = ArenaStack::default();
            &mut fresh_stack
        }
    };
    let ordinal = stack
        .stack
        .layers
        .iter()
        .filter(|layer| matches!(layer.kind, LayerKind::Tiles(_)))
        .count()
        .strict_add(1);
    let id = stack.stack.next_id();
    stack.stack.layers.push(Layer::new(
        id,
        format!("Tiles {ordinal}"),
        LayerKind::Tiles(Vec::new()),
    ));
    editor.selected_layer = Some(id);
    info!("{}: new tile layer \"Tiles {ordinal}\"", arena.name());
    if created {
        commands.entity(arena_entity).insert(stack.clone());
    }
}

/// `F5` — restart the current arena (clock to 0, ghosts to their starts) for a
/// fresh watch-through of the latest take.
fn restart_current(
    current: Res<CurrentArena>,
    mut arenas: Query<(Entity, &Arena, &mut ArenaClock, &mut ArenaTimeline)>,
    mut ghosts: Query<(Entity, &Ghost, &ChildOf, &mut TileMover, &mut Transform)>,
) {
    let Some((arena_entity, _, mut clock, mut timeline)) = arenas
        .iter_mut()
        .find(|(_, arena, ..)| arena.index() == current.0)
    else {
        return;
    };
    restart(&mut clock, &mut timeline);
    snap_arena_ghosts(arena_entity, &mut ghosts, None);
}

/// The author chip, top-right under the bar — a sibling of the REC strip.
fn setup_author_hud(mut commands: Commands, mono: Res<MonoFont>, current: Res<CurrentArena>) {
    let theme = Arena::from_index(current.0)
        .expect("invariant: CurrentArena is a valid arena index")
        .theme()
        .palette();
    commands.spawn((
        DespawnOnExit(AppState::Intro),
        GlobalZIndex(6),
        Pickable::IGNORE,
        Node {
            position_type: PositionType::Absolute,
            top: Val::Px(TOP_BAR_PX + 8.0),
            right: Val::Px(12.0),
            ..default()
        },
        children![(
            AuthorHud,
            Text::new("AUTHOR"),
            TextFont {
                font: mono.0.clone(),
                font_size: 12.0,
                ..default()
            },
            TextColor(theme.warning),
        )],
    ));
}

/// Keeps the chip current: difficulty, focused arena, its loaded score
/// versions, and — while editing — the cursor cell + in/out marks, themed to
/// the focused arena.
fn update_author_hud(
    current: Res<CurrentArena>,
    difficulty: Res<ActiveDifficulty>,
    editing: Option<Res<TileEditMode>>,
    editor: Res<TileEditor>,
    stacks: Query<(&Arena, &ArenaStack)>,
    hud: Single<(&mut Text, &mut TextColor), With<AuthorHud>>,
) {
    let arena =
        Arena::from_index(current.0).expect("invariant: CurrentArena is a valid arena index");
    let stack = stacks
        .iter()
        .find(|(a, _)| a.index() == current.0)
        .map(|(_, stack)| stack);
    let summary = match stack {
        Some(stack) => {
            let version = stack
                .version
                .map_or("draft".to_string(), |v| format!("v{v}"));
            let dirty = stack
                .stack
                .layers
                .iter()
                .chain(&stack.published.layers)
                .map(|layer| layer.id)
                .collect::<std::collections::BTreeSet<_>>()
                .into_iter()
                .filter(|&id| stack.layer_dirty(id))
                .count();
            let mut s = format!("stack {version} · {} layers", stack.stack.layers.len());
            if dirty > 0 {
                s.push_str(&format!(" ({dirty} dirty)"));
            }
            s
        }
        None => "no stack".to_string(),
    };
    let mut line = format!(
        "AUTHOR · {} · {} · {summary}",
        difficulty.0.slug().to_uppercase(),
        arena.slug(),
    );
    if editing.is_some() {
        let layer_name = editor
            .selected_layer
            .and_then(|id| stack.and_then(|s| s.stack.layer(id)))
            .map_or("—", |layer| layer.name.as_str());
        line.push_str(&format!(
            " · TILE ({},{}) [{}→{}] → {layer_name}",
            editor.cursor.x,
            editor.cursor.y,
            fmt_tick(editor.mark_in),
            fmt_tick(editor.mark_out),
        ));
    }
    let (mut text, mut color) = hud.into_inner();
    // Write-if-changed, like the REC strip — no per-frame glyph reshaping.
    if text.0 != line {
        text.0 = line;
    }
    let warning = arena.theme().palette().warning;
    if color.0 != warning {
        color.0 = warning;
    }
}

/// Leaving the intro drops the editor flag (entities despawn via
/// `DespawnOnExit`); without this the world-input gates would stay locked on
/// the next visit.
fn reset_on_exit(mut commands: Commands) {
    commands.remove_resource::<TileEditMode>();
}

/// The author-mode toolkit. Compiled only with the `author` feature; nothing
/// here ships.
pub(crate) struct AuthorPlugin;

impl Plugin for AuthorPlugin {
    fn build(&self, app: &mut App) {
        app.init_resource::<TileEditor>()
            .add_systems(OnEnter(AppState::Intro), setup_author_hud)
            .add_systems(OnExit(AppState::Intro), reset_on_exit)
            .add_systems(
                Update,
                (
                    possess_boss.run_if(
                        input_just_pressed(KeyCode::KeyB)
                            .and(no_modal)
                            .and(is_idle)
                            .and(no_pending_walk)
                            .and(not_tile_editing),
                    ),
                    cycle_difficulty.run_if(
                        input_just_pressed(KeyCode::KeyD)
                            .and(no_modal)
                            .and(is_idle)
                            .and(not_tile_editing),
                    ),
                    boss_cast_slots.run_if(
                        input_just_pressed(KeyCode::Digit1)
                            .or(input_just_pressed(KeyCode::Numpad1))
                            .or(input_just_pressed(KeyCode::Digit2))
                            .or(input_just_pressed(KeyCode::Numpad2))
                            .or(input_just_pressed(KeyCode::Digit3))
                            .or(input_just_pressed(KeyCode::Numpad3))
                            .or(input_just_pressed(KeyCode::Digit4))
                            .or(input_just_pressed(KeyCode::Numpad4))
                            .and(no_modal)
                            .and(not_counting_down)
                            .and(no_pending_walk)
                            .and(not_tile_editing),
                    ),
                    commit_to_layer.run_if(on_message::<RecordingCommitted>),
                    // `W` publishes the focused arena's whole stack — from
                    // anywhere in author mode, tile editor open or not.
                    publish_stack
                        .run_if(input_just_pressed(KeyCode::KeyW).and(no_modal).and(is_idle)),
                    toggle_tile_mode.run_if(
                        input_just_pressed(KeyCode::KeyT)
                            .and(no_modal)
                            .and(is_idle)
                            .and(no_pending_walk),
                    ),
                    restart_current.run_if(
                        input_just_pressed(KeyCode::F5)
                            .and(no_modal)
                            .and(is_idle)
                            .and(not_tile_editing),
                    ),
                    // The editor's own keys — only while it is open (and no
                    // modal): the cursor, the scrub, the marks, the paint, the
                    // save.
                    (
                        move_cursor.run_if(arrow_pressed),
                        scrub.run_if(
                            input_just_pressed(KeyCode::Period)
                                .or(input_just_pressed(KeyCode::Comma)),
                        ),
                        set_marks.run_if(
                            input_just_pressed(KeyCode::KeyI).or(input_just_pressed(KeyCode::KeyO)),
                        ),
                        paint.run_if(input_just_pressed(KeyCode::Space)),
                        cycle_tile_layer.run_if(input_just_pressed(KeyCode::Tab)),
                        new_tile_layer.run_if(input_just_pressed(KeyCode::KeyN)),
                    )
                        .run_if(tile_editing.and(no_modal)),
                    // The chip re-renders only when one of its inputs moved —
                    // its strings are rebuilt on demand, not per frame.
                    update_author_hud.run_if(
                        resource_changed::<CurrentArena>
                            .or(resource_changed::<ActiveDifficulty>)
                            .or(resource_changed::<TileEditor>)
                            .or(resource_added::<TileEditMode>)
                            .or(resource_removed::<TileEditMode>)
                            .or(stacks_changed),
                    ),
                )
                    .run_if(in_state(AppState::Intro)),
            );
    }
}
