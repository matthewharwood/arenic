//! Small themed UI builders shared by the stories. Every builder pulls its
//! colours from a [`Theme`] so the whole style guide re-tones when the active
//! theme changes.

use arenic_game::theme::{Theme, scale};
use bevy::prelude::*;

/// A text node at an explicit size and colour.
pub fn label(commands: &mut Commands, s: &str, size: f32, color: Color) -> Entity {
    commands
        .spawn((
            Text::new(s),
            TextFont {
                font_size: size,
                ..default()
            },
            TextColor(color),
        ))
        .id()
}

/// A vertical stack with a fixed row gap.
pub fn column(commands: &mut Commands, gap: f32) -> Entity {
    commands
        .spawn(Node {
            flex_direction: FlexDirection::Column,
            row_gap: Val::Px(gap),
            ..default()
        })
        .id()
}

/// A horizontal, centre-aligned stack with a fixed column gap.
pub fn row(commands: &mut Commands, gap: f32) -> Entity {
    commands
        .spawn(Node {
            flex_direction: FlexDirection::Row,
            align_items: AlignItems::Center,
            column_gap: Val::Px(gap),
            ..default()
        })
        .id()
}

/// A wrapping horizontal stack — for swatch grids that reflow.
pub fn wrap(commands: &mut Commands, gap: f32) -> Entity {
    commands
        .spawn(Node {
            flex_direction: FlexDirection::Row,
            flex_wrap: FlexWrap::Wrap,
            align_items: AlignItems::FlexStart,
            column_gap: Val::Px(gap),
            row_gap: Val::Px(gap),
            ..default()
        })
        .id()
}

/// A story section heading (large, primary text colour).
pub fn heading(commands: &mut Commands, theme: &Theme, s: &str) -> Entity {
    label(commands, s, scale::font_size::F4, theme.text_1())
}

/// A sub-section title (muted, small-caps feel via the muted tone).
pub fn subheading(commands: &mut Commands, theme: &Theme, s: &str) -> Entity {
    label(commands, s, scale::font_size::F1, theme.text_2())
}

/// A colour swatch: a rounded chip of `color` with a name + value caption.
pub fn swatch(
    commands: &mut Commands,
    theme: &Theme,
    color: Color,
    name: &str,
    value: &str,
) -> Entity {
    let col = column(commands, scale::space::XS3);

    let chip = commands
        .spawn((
            Node {
                width: Val::Px(96.0),
                height: Val::Px(56.0),
                border: UiRect::all(Val::Px(1.0)),
                border_radius: BorderRadius::all(Val::Px(scale::radius::S)),
                ..default()
            },
            BackgroundColor(color),
            BorderColor::all(theme.border_subtle()),
        ))
        .id();

    let name = label(commands, name, scale::font_size::F00, theme.text_1());
    let value = label(commands, value, scale::font_size::F00, theme.text_muted());

    commands.entity(col).add_children(&[chip, name, value]);
    col
}

/// Formats a colour as a `#RRGGBB` sRGB hex string.
pub fn hex(color: Color) -> String {
    let c = color.to_srgba();
    format!(
        "#{:02X}{:02X}{:02X}",
        (c.red * 255.0).round() as u8,
        (c.green * 255.0).round() as u8,
        (c.blue * 255.0).round() as u8,
    )
}

/// A titled group: a subheading above a wrapping grid of `children`.
pub fn group(commands: &mut Commands, theme: &Theme, title: &str, children: &[Entity]) -> Entity {
    let col = column(commands, scale::space::XS);
    let title = subheading(commands, theme, title);
    let grid = wrap(commands, scale::space::S);
    commands.entity(grid).add_children(children);
    commands.entity(col).add_children(&[title, grid]);
    col
}
