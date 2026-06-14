//! **Layer CRUD + save events** — the one funnel for editing the authoring
//! "entities" (layers) of an arena's DRAFT [`ArenaStack`]. Mirrors the
//! [`arenic_game::tile::TileBoard`] SystemParam pattern: callers take
//! [`LayerEditor`] and call a method; it resolves the target arena's draft
//! (creating one if the arena has none), mutates it, fires the [`Saved`] event
//! on a persisted change, and re-folds the preview when the *effective* stack
//! changed — so every edit funnels through one place instead of ad-hoc
//! `layer_mut` calls scattered across the author tooling.
//!
//! "Save" here means **committed to the draft** (auto-dirty via `layer_dirty`,
//! which counts `name`/`muted`/`kind`/`effects`). The durable on-disk write is
//! still `W` (the versioned `layers.vNNNN.ron` publish); a [`Saved`] just marks
//! the commit and drives a brief "✓ saved" badge. Author-feature only.

use arenic_game::arena::Arena;
use arenic_game::default_font::MonoFont;
use arenic_game::layer::{ArenaStack, LayerId};
use arenic_game::timeline::ArenaClock;
use bevy::color::Alpha;
use bevy::ecs::system::SystemParam;
use bevy::prelude::*;

use crate::author::{push_minion_layer, push_tile_layer};
use crate::dope_sheet::{RefoldPreview, SheetBody};
use crate::states::AppState;

/// A layer edit was committed to the draft (the "save event"). Fired only for
/// PERSISTED changes (rename / mute / create / delete) — `solo`/`locked` are
/// session UI and don't claim a save.
#[derive(Message, Clone, Copy)]
pub(crate) struct Saved {
    pub(crate) arena: usize,
    pub(crate) layer: LayerId,
}

/// The reusable layer CRUD API. Every method targets `arena` (an index — the
/// focused arena for keyboard ops, the dropped arena for drag-to-board).
#[derive(SystemParam)]
pub(crate) struct LayerEditor<'w, 's> {
    arenas: Query<
        'w,
        's,
        (
            Entity,
            &'static Arena,
            &'static ArenaClock,
            Option<&'static mut ArenaStack>,
        ),
    >,
    commands: Commands<'w, 's>,
    saved: MessageWriter<'w, Saved>,
    refold: MessageWriter<'w, RefoldPreview>,
}

impl LayerEditor<'_, '_> {
    /// Resolves `arena`'s draft stack (inserting a fresh one if it has none),
    /// runs `edit` against it, and returns `(arena entity, edit result)`.
    fn with_stack<R>(
        &mut self,
        arena: usize,
        edit: impl FnOnce(&mut ArenaStack, u32) -> R,
    ) -> Option<(Entity, R)> {
        let (entity, tick) = self
            .arenas
            .iter()
            .find(|(_, candidate, ..)| candidate.index() == arena)
            .map(|(entity, _, clock, _)| (entity, clock.tick))?;
        let stack = self
            .arenas
            .get_mut(entity)
            .ok()
            .and_then(|(.., stack)| stack);
        let result = match stack {
            Some(mut stack) => edit(&mut stack, tick),
            None => {
                // Never-stacked arena: edit a fresh stack, then attach it.
                let mut fresh = ArenaStack::default();
                let result = edit(&mut fresh, tick);
                self.commands.entity(entity).insert(fresh);
                result
            }
        };
        Some((entity, result))
    }

    // --- READ ----------------------------------------------------------------

    /// The current name of `layer` in `arena`'s draft (CRUD's read).
    pub(crate) fn layer_name(&self, arena: usize, layer: LayerId) -> Option<String> {
        self.arenas
            .iter()
            .find(|(_, candidate, ..)| candidate.index() == arena)
            .and_then(|(.., stack)| stack)
            .and_then(|stack| stack.stack.layer(layer))
            .map(|layer| layer.name.clone())
    }

    // --- UPDATE --------------------------------------------------------------

    /// Renames `layer` (the authoring alias only — never its id/binding). Fires
    /// [`Saved`] when the name actually changes.
    pub(crate) fn rename(&mut self, arena: usize, layer: LayerId, name: &str) {
        let Some((_, changed)) =
            self.with_stack(arena, |stack, _| match stack.stack.layer_mut(layer) {
                Some(target) if target.name != name => {
                    target.name = name.to_owned();
                    true
                }
                _ => false,
            })
        else {
            return;
        };
        if changed {
            self.saved.write(Saved { arena, layer });
        }
    }

    /// Flips `muted` (persisted → save + re-fold).
    pub(crate) fn toggle_muted(&mut self, arena: usize, layer: LayerId) {
        if let Some((entity, true)) = self.flip(arena, layer, |target| &mut target.muted) {
            self.saved.write(Saved { arena, layer });
            self.refold.write(RefoldPreview { arena: entity });
        }
    }

    /// Flips `solo` (session UI → re-fold only, no save).
    pub(crate) fn toggle_solo(&mut self, arena: usize, layer: LayerId) {
        if let Some((entity, true)) = self.flip(arena, layer, |target| &mut target.solo) {
            self.refold.write(RefoldPreview { arena: entity });
        }
    }

    /// Flips `locked` (session UI → neither save nor re-fold).
    pub(crate) fn toggle_locked(&mut self, arena: usize, layer: LayerId) {
        self.flip(arena, layer, |target| &mut target.locked);
    }

    fn flip(
        &mut self,
        arena: usize,
        layer: LayerId,
        field: impl FnOnce(&mut arenic_game::layer::Layer) -> &mut bool,
    ) -> Option<(Entity, bool)> {
        self.with_stack(arena, |stack, _| match stack.stack.layer_mut(layer) {
            Some(target) => {
                let flag = field(target);
                *flag = !*flag;
                true
            }
            None => false,
        })
    }

    // --- DELETE --------------------------------------------------------------

    /// Removes `layer` from the draft (the entity binding + replay clean up via
    /// the re-fold). Fires [`Saved`] when a layer was actually removed.
    pub(crate) fn delete(&mut self, arena: usize, layer: LayerId) {
        let Some((entity, removed)) = self.with_stack(arena, |stack, _| {
            let before = stack.stack.layers.len();
            stack.stack.layers.retain(|candidate| candidate.id != layer);
            stack.stack.layers.len() != before
        }) else {
            return;
        };
        if removed {
            self.saved.write(Saved { arena, layer });
            self.refold.write(RefoldPreview { arena: entity });
        }
    }

    // --- CREATE --------------------------------------------------------------

    /// Adds a minion layer at `tile`, alive from the arena's playhead tick.
    /// Returns its new id.
    pub(crate) fn create_minion(&mut self, arena: usize, tile: IVec2) -> Option<LayerId> {
        let (entity, id) =
            self.with_stack(arena, |stack, tick| push_minion_layer(stack, tick, tile))?;
        self.saved.write(Saved { arena, layer: id });
        self.refold.write(RefoldPreview { arena: entity });
        Some(id)
    }

    /// Adds an empty tile layer on top of the stack. Returns its new id.
    pub(crate) fn create_tile_layer(&mut self, arena: usize) -> Option<LayerId> {
        let (entity, id) = self.with_stack(arena, |stack, _| push_tile_layer(stack))?;
        self.saved.write(Saved { arena, layer: id });
        self.refold.write(RefoldPreview { arena: entity });
        Some(id)
    }
}

/// How long the "✓ saved" badge lingers (seconds).
const BADGE_SECS: f32 = 0.9;

/// The transient save-confirmation badge; its remaining lifetime in seconds.
#[derive(Component)]
struct SaveBadge(f32);

/// Shows a brief "✓ saved" badge in the dope-sheet panel on every [`Saved`],
/// then fades it out — the visible half of the save event.
fn save_feedback(
    mut events: MessageReader<Saved>,
    time: Res<Time>,
    mut commands: Commands,
    mono: Res<MonoFont>,
    body: Option<Single<Entity, With<SheetBody>>>,
    badge: Option<Single<(Entity, &mut SaveBadge, &mut TextColor)>>,
) {
    let latest = events.read().last().copied();
    if let Some(saved) = latest {
        info!("layer save: {:?} in arena {}", saved.layer, saved.arena);
    }
    match badge {
        Some(badge) => {
            let (entity, mut life, mut color) = badge.into_inner();
            if latest.is_some() {
                life.0 = BADGE_SECS;
            }
            life.0 -= time.delta_secs();
            if life.0 <= 0.0 {
                commands.entity(entity).despawn();
            } else {
                color.0 = color.0.with_alpha((life.0 / BADGE_SECS).clamp(0.0, 1.0));
            }
        }
        None => {
            let Some(saved) = latest else {
                return;
            };
            let Some(body) = body else {
                return;
            };
            let color = Arena::from_index(saved.arena)
                .map_or(Color::WHITE, |arena| arena.theme().palette().primary);
            commands.spawn((
                SaveBadge(BADGE_SECS),
                ChildOf(*body),
                GlobalZIndex(12),
                Text::new("\u{2713} saved"),
                TextFont {
                    font: mono.0.clone(),
                    font_size: 10.0,
                    ..default()
                },
                TextColor(color),
                Node {
                    position_type: PositionType::Absolute,
                    top: Val::Px(5.0),
                    right: Val::Px(10.0),
                    ..default()
                },
            ));
        }
    }
}

/// Registers the [`Saved`] event + its feedback badge. The CRUD methods live on
/// the [`LayerEditor`] SystemParam, which any author system can take as a param.
pub(crate) struct LayerEditPlugin;

impl Plugin for LayerEditPlugin {
    fn build(&self, app: &mut App) {
        app.add_message::<Saved>()
            .add_systems(Update, save_feedback.run_if(in_state(AppState::Intro)));
    }
}
