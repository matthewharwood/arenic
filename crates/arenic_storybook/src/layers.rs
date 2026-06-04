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
    /// Every layer with its checkbox label, in display order. This is the ONE
    /// place layers are enumerated — [`LayerVisibility`] is indexed by it, so a new
    /// layer is a single enum variant + a row here, nothing else.
    pub const ALL: [(Layer, &'static str); 6] = [
        (Layer::Skybox, "Skybox"),
        (Layer::Foreground, "Foreground"),
        (Layer::Swarm, "Swarm"),
        (Layer::Boss, "Boss"),
        (Layer::Floor, "Floor / Ground"),
        (Layer::Props, "Flora / Fauna"),
    ];

    /// The number of layers, and each layer's index into [`LayerVisibility`].
    /// `as usize` follows declaration order, which [`Self::ALL`] mirrors (pinned by
    /// a test).
    pub const COUNT: usize = Self::ALL.len();
    fn index(self) -> usize {
        self as usize
    }
}

/// Tags an entity as belonging to a toggleable [`Layer`].
#[derive(Component, Clone, Copy)]
pub struct LayerTag(pub Layer);

/// Which layers are currently visible — one flag per [`Layer`], indexed by it
/// (no parallel field-per-layer struct to keep in sync). All on by default.
#[derive(Resource, Clone, Copy)]
pub struct LayerVisibility([bool; Layer::COUNT]);

impl Default for LayerVisibility {
    fn default() -> Self {
        Self([true; Layer::COUNT])
    }
}

impl LayerVisibility {
    pub fn get(&self, layer: Layer) -> bool {
        self.0[layer.index()]
    }

    pub fn toggle(&mut self, layer: Layer) {
        let i = layer.index();
        self.0[i] = !self.0[i];
    }
}

pub struct LayersPlugin;

impl Plugin for LayersPlugin {
    fn build(&self, app: &mut App) {
        app.init_resource::<LayerVisibility>().add_systems(
            Update,
            (
                // Re-scan everything ONLY when a checkbox flips...
                reconcile_layers.run_if(resource_changed::<LayerVisibility>),
                // ...otherwise just adopt the current toggles onto entities spawned
                // this frame (per-story boss/swarm/props), so the full 2000+-entity
                // scan never runs on an idle frame.
                init_new_layers,
            ),
        );
    }
}

fn visibility_for(visible: bool) -> Visibility {
    if visible {
        Visibility::Inherited
    } else {
        Visibility::Hidden
    }
}

/// Reconciles every tagged entity's `Visibility` when the toggle set changes.
fn reconcile_layers(vis: Res<LayerVisibility>, mut tagged: Query<(&LayerTag, &mut Visibility)>) {
    for (tag, mut visibility) in &mut tagged {
        let want = visibility_for(vis.get(tag.0));
        if *visibility != want {
            *visibility = want;
        }
    }
}

/// Applies the current toggles to entities spawned this frame, so freshly-spawned
/// content honours a layer that is currently toggled off — without a full re-scan.
fn init_new_layers(
    vis: Res<LayerVisibility>,
    mut tagged: Query<(&LayerTag, &mut Visibility), Added<LayerTag>>,
) {
    for (tag, mut visibility) in &mut tagged {
        let want = visibility_for(vis.get(tag.0));
        if *visibility != want {
            *visibility = want;
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn all_layers_listed_in_index_order() {
        // LayerVisibility indexes by `Layer as usize`; ALL must mirror that order,
        // or get/toggle would read the wrong flag.
        for (i, (layer, _)) in Layer::ALL.iter().enumerate() {
            assert_eq!(layer.index(), i, "Layer::ALL row {i} is out of index order");
        }
        assert_eq!(Layer::COUNT, Layer::ALL.len());
    }

    #[test]
    fn default_is_all_on_and_toggle_flips_one_layer() {
        let mut v = LayerVisibility::default();
        assert!(
            Layer::ALL.iter().all(|&(l, _)| v.get(l)),
            "default must be all-on"
        );
        v.toggle(Layer::Boss);
        assert!(!v.get(Layer::Boss));
        assert!(v.get(Layer::Skybox), "toggling Boss must not touch Skybox");
    }
}
