//! The storybook's `StoryId → shared arena` bridge.
//!
//! Every arena's identity AND its six-layer content (theme, voices, swarm, boss,
//! props) lives once in [`arenic_game::arena`], read by both binaries. This module
//! is just the bridge — which [`Arena`] each storybook [`StoryId`] stands in — plus
//! the generic atmosphere for the non-arena (design-token) pages.

use arenic_game::arena::{Arena, ArenaSpec};
use arenic_game::atmosphere::{CloudStyle, Voice};
use bevy::prelude::*;

use crate::stories::StoryId;

/// The atmosphere for non-arena stories (the design-token pages): a calm, generic
/// sky + near haze so the style guide still has depth. (Arena stories read their
/// voices from [`Arena::voices`] instead.)
pub(crate) const DEFAULT_SKY: Voice = Voice {
    style: CloudStyle::Billows,
    scale: 2.5,
    drift: Vec2::new(0.010, 0.010),
    coverage: 0.50,
    vignette: 0.35,
    speed: 0.60,
};
pub(crate) const DEFAULT_FG: Voice = Voice {
    style: CloudStyle::Grain,
    scale: 7.0,
    drift: Vec2::new(-0.010, 0.000),
    coverage: 0.40,
    vignette: 0.0,
    speed: 0.80,
};

/// The [`Arena`] a story stands in, or `None` for the non-arena (design-token) pages.
fn arena_of(story: StoryId) -> Option<Arena> {
    match story {
        StoryId::Hunter => Some(Arena::Hunter),
        StoryId::Guildmaster => Some(Arena::Guildmaster),
        StoryId::Cardinal => Some(Arena::Cardinal),
        StoryId::Forager => Some(Arena::Forager),
        StoryId::Warrior => Some(Arena::Warrior),
        StoryId::Thief => Some(Arena::Thief),
        StoryId::Alchemist => Some(Arena::Alchemist),
        StoryId::Merchant => Some(Arena::Merchant),
        StoryId::Bard => Some(Arena::Bard),
        StoryId::Colors
        | StoryId::Typography
        | StoryId::Spacing
        | StoryId::Radii
        | StoryId::Elevation
        | StoryId::Buttons
        | StoryId::ActionBar
        | StoryId::HolyNova => None,
    }
}

/// The full spec for `story` (theme + voices + swarm/boss/props), or `None` for
/// non-arena stories. Sourced entirely from [`arenic_game::arena`].
pub(crate) fn spec(story: StoryId) -> Option<ArenaSpec> {
    arena_of(story).map(Arena::spec)
}

#[cfg(test)]
mod tests {
    use super::*;

    /// The nine arena stories (the 3×3 grid).
    const ARENA_STORIES: [StoryId; 9] = [
        StoryId::Hunter,
        StoryId::Guildmaster,
        StoryId::Cardinal,
        StoryId::Forager,
        StoryId::Warrior,
        StoryId::Thief,
        StoryId::Alchemist,
        StoryId::Merchant,
        StoryId::Bard,
    ];

    #[test]
    fn every_arena_has_a_spec() {
        for (i, s) in ARENA_STORIES.into_iter().enumerate() {
            assert!(
                spec(s).is_some(),
                "arena story #{i} is missing an ArenaSpec"
            );
        }
    }

    #[test]
    fn non_arena_story_has_no_spec() {
        assert!(spec(StoryId::Colors).is_none());
    }

    #[test]
    fn maps_to_the_matching_shared_arena() {
        assert_eq!(spec(StoryId::Hunter).unwrap().theme, Arena::Hunter.theme());
    }
}
