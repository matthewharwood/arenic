//! Stage **layer visibility** — small checkboxes in the storybook toggle each
//! visual layer of the shared 3D stage on and off (all ON by default).
//!
//! Every toggleable entity carries a [`LayerTag`]; one system reconciles each
//! entity's `Visibility` from the [`LayerVisibility`] resource, so toggles also
//! apply to per-story respawns (boss / swarm / props) and persist across stories.

use bevy::prelude::*;

/// The toggleable stage layers, matching the storybook's checkbox set.
#[derive(Clone, Copy, PartialEq, Eq, Hash, Debug)]
pub enum Layer {
    Skybox,
    Foreground,
    Swarm,
    Boss,
    Floor,
    Props,
}

impl Layer {
    /// Every layer with its checkbox label, in display order.
    pub const ALL: [(Layer, &'static str); 6] = [
        (Layer::Skybox, "Skybox"),
        (Layer::Foreground, "Foreground"),
        (Layer::Swarm, "Swarm"),
        (Layer::Boss, "Boss"),
        (Layer::Floor, "Floor / Ground"),
        (Layer::Props, "Flora / Fauna"),
    ];
}

/// Tags an entity as belonging to a toggleable [`Layer`].
#[derive(Component, Clone, Copy)]
pub struct LayerTag(pub Layer);

/// Which layers are currently visible. All on by default.
#[derive(Resource, Clone, Copy)]
pub struct LayerVisibility {
    pub skybox: bool,
    pub foreground: bool,
    pub swarm: bool,
    pub boss: bool,
    pub floor: bool,
    pub props: bool,
}

impl Default for LayerVisibility {
    fn default() -> Self {
        Self {
            skybox: true,
            foreground: true,
            swarm: true,
            boss: true,
            floor: true,
            props: true,
        }
    }
}

impl LayerVisibility {
    pub fn get(&self, layer: Layer) -> bool {
        match layer {
            Layer::Skybox => self.skybox,
            Layer::Foreground => self.foreground,
            Layer::Swarm => self.swarm,
            Layer::Boss => self.boss,
            Layer::Floor => self.floor,
            Layer::Props => self.props,
        }
    }

    pub fn toggle(&mut self, layer: Layer) {
        match layer {
            Layer::Skybox => self.skybox = !self.skybox,
            Layer::Foreground => self.foreground = !self.foreground,
            Layer::Swarm => self.swarm = !self.swarm,
            Layer::Boss => self.boss = !self.boss,
            Layer::Floor => self.floor = !self.floor,
            Layer::Props => self.props = !self.props,
        }
    }
}

pub struct LayersPlugin;

impl Plugin for LayersPlugin {
    fn build(&self, app: &mut App) {
        app.init_resource::<LayerVisibility>()
            .add_systems(Update, apply_layer_visibility);
    }
}

/// Reconciles every tagged entity's `Visibility` from [`LayerVisibility`]. Runs
/// every frame but only writes when a value actually differs, so it covers both
/// checkbox toggles and freshly-spawned (per-story) entities without thrash.
fn apply_layer_visibility(vis: Res<LayerVisibility>, mut tagged: Query<(&LayerTag, &mut Visibility)>) {
    for (tag, mut visibility) in &mut tagged {
        let want = if vis.get(tag.0) {
            Visibility::Inherited
        } else {
            Visibility::Hidden
        };
        if *visibility != want {
            *visibility = want;
        }
    }
}
