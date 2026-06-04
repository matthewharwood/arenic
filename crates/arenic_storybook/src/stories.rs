//! The stories rendered into the canvas. The `styles/*` stories are the design-
//! token style guide; the `units/*` stories (`guildmaster`, `boss`) drop a real
//! `arenic_game` piece onto the shared top-down 3D stage (see [`crate::stage`]).
//! Each builder fills a `content` column using the active [`Theme`].

use arenic_game::interaction::{Interactive, hidden_outline};
use arenic_game::theme::{Theme, ThemeId, scale};
use bevy::prelude::*;

use crate::widgets::{self, column, heading, hex, label, row, swatch, wrap};

/// Every selectable story.
#[derive(Clone, Copy, PartialEq, Eq, Hash, Debug)]
pub enum StoryId {
    Colors,
    Typography,
    Spacing,
    Radii,
    Elevation,
    Components,
    // The nine arenas, in 3×3 grid order (class — arena); see the rulebook.
    Guildmaster,
    Hunter,
    Alchemist,
    Cardinal,
    Warrior,
    Thief,
    Bard,
    Forager,
    Merchant,
}

impl StoryId {
    pub fn title(self) -> &'static str {
        match self {
            StoryId::Colors => "Colors",
            StoryId::Typography => "Typography",
            StoryId::Spacing => "Spacing",
            StoryId::Radii => "Radii",
            StoryId::Elevation => "Elevation",
            StoryId::Components => "Components",
            StoryId::Guildmaster => "Guildmaster — Guild House",
            StoryId::Hunter => "Hunter — Labyrinth",
            StoryId::Alchemist => "Alchemist — Crucible",
            StoryId::Cardinal => "Cardinal — Sanctum",
            StoryId::Warrior => "Warrior — Bastion",
            StoryId::Thief => "Thief — Pawnshop",
            StoryId::Bard => "Bard — Gala",
            StoryId::Forager => "Forager — Mountain",
            StoryId::Merchant => "Merchant — Casino",
        }
    }

    /// Whether this story renders the 3D stage (and so wants the orbit control).
    /// Everything outside the `styles/*` design-token set is a 3D unit/boss
    /// story; the stage itself (camera/light/shadow/RTT) is shared — these
    /// stories supply only the content on the board (see [`crate::stage`]).
    pub fn has_3d_stage(self) -> bool {
        !matches!(
            self,
            StoryId::Colors
                | StoryId::Typography
                | StoryId::Spacing
                | StoryId::Radii
                | StoryId::Elevation
                | StoryId::Components
        )
    }

    /// The matched theme for an arena story — opening it renders the boss (and
    /// the whole stage) in this palette, so each boss reads on-theme. Returns
    /// `None` for the non-arena stories (styles, guildmaster, generic boss).
    pub fn arena_theme(self) -> Option<ThemeId> {
        Some(match self {
            StoryId::Hunter => ThemeId::TokyoNight,
            StoryId::Guildmaster => ThemeId::Coffee,
            StoryId::Cardinal => ThemeId::Luxury,
            StoryId::Forager => ThemeId::Forest,
            StoryId::Warrior => ThemeId::GruvboxDark,
            StoryId::Thief => ThemeId::AyuDark,
            StoryId::Alchemist => ThemeId::Abyss,
            StoryId::Merchant => ThemeId::RosePine,
            StoryId::Bard => ThemeId::Synthwave,
            _ => return None,
        })
    }
}

/// Renders `story` (or a placeholder) into the `content` column. `stage` is the
/// shared 3D render-to-texture that the `units/*` (3D) stories display.
pub fn render(
    commands: &mut Commands,
    content: Entity,
    theme: &Theme,
    story: Option<StoryId>,
    stage: &Handle<Image>,
) {
    let Some(story) = story else {
        let hint = label(
            commands,
            "Select a story",
            scale::font_size::F2,
            theme.text_muted(),
        );
        commands.entity(content).add_children(&[hint]);
        return;
    };

    let title = heading(commands, theme, story.title());
    commands.entity(content).add_children(&[title]);

    match story {
        StoryId::Colors => colors(commands, content, theme),
        StoryId::Typography => typography(commands, content, theme),
        StoryId::Spacing => spacing(commands, content, theme),
        StoryId::Radii => radii(commands, content, theme),
        StoryId::Elevation => elevation(commands, content, theme),
        StoryId::Components => components(commands, content, theme),
        StoryId::Guildmaster => stage_story(
            commands,
            content,
            theme,
            stage,
            "Guild House / Guildmaster \u{2014} home. The guild's safe hearth, not a true \
             boss: a calm pyramid \u{2018}home\u{2019} at centre on the warm Coffee theme \
             (the home is the one bright, safe arena). Unlock the camera to orbit.",
        ),
        StoryId::Hunter => stage_story(
            commands, content, theme, stage,
            "Labyrinth / Hunter \u{2014} Hollow Obelisk. A dark hollow relic; a blade of \
             light sweeps the inner walls as a sniper telegraph. The void glows, the shell \
             stays dark. Press [T] to cycle Normal/Heroic/Mythic; unlock the camera to orbit.",
        ),
        StoryId::Alchemist => stage_story(
            commands, content, theme, stage,
            "Crucible / Alchemist \u{2014} Truncated Cone (the vessel). Liquid light rises \
             and falls inside the vessel. The void glows, the shell stays dark. Press [T] to \
             cycle Normal/Heroic/Mythic; unlock the camera to orbit.",
        ),
        StoryId::Cardinal => stage_story(
            commands, content, theme, stage,
            "Sanctum / Cardinal \u{2014} Torus Halo. The light lives inside the ring's hole; \
             it fills, resolves, then resets (ability-ready telegraph). Press [T] to cycle \
             Normal/Heroic/Mythic; unlock the camera to orbit.",
        ),
        StoryId::Warrior => stage_story(
            commands, content, theme, stage,
            "Bastion / Warrior \u{2014} Hexagonal Prism (bunker). A compressed furnace core \
             lights a blocked direction. The void glows, the shell stays dark. Press [T] to \
             cycle Normal/Heroic/Mythic; unlock the camera to orbit.",
        ),
        StoryId::Thief => stage_story(
            commands, content, theme, stage,
            "Pawnshop / Thief \u{2014} Triangular Prism (wedge). A narrow beam projects forward \
             \u{2014} a slit in reality, not a lamp. Press [T] to cycle Normal/Heroic/Mythic; \
             unlock the camera to orbit.",
        ),
        StoryId::Bard => stage_story(
            commands, content, theme, stage,
            "Gala / Bard \u{2014} Capsule Resonator. A central filament pulses on the beat \
             \u{2014} sonic geometry. The void glows, the shell stays dark. Press [T] to cycle \
             Normal/Heroic/Mythic; unlock the camera to orbit.",
        ),
        StoryId::Forager => stage_story(
            commands, content, theme, stage,
            "Mountain / Forager \u{2014} Stepped Pyramid (ziggurat). Light grows upward from \
             the central shaft. The void glows, the shell stays dark. Press [T] to cycle \
             Normal/Heroic/Mythic; unlock the camera to orbit.",
        ),
        StoryId::Merchant => stage_story(
            commands, content, theme, stage,
            "Casino / Merchant \u{2014} Hollow Icosphere (geode). A crystalline core shimmers \
             semi-randomly \u{2014} luck and volatility. Press [T] to cycle Normal/Heroic/\
             Mythic; unlock the camera to orbit.",
        ),
    }
}

fn colors(commands: &mut Commands, content: Entity, theme: &Theme) {
    // Primitive palette.
    let base = [
        (theme.base_100, "base-100"),
        (theme.base_200, "base-200"),
        (theme.base_300, "base-300"),
        (theme.base_content, "base-content"),
    ];
    let brand = [
        (theme.primary, "primary"),
        (theme.primary_content, "primary-c"),
        (theme.secondary, "secondary"),
        (theme.secondary_content, "secondary-c"),
        (theme.accent, "accent"),
        (theme.accent_content, "accent-c"),
        (theme.neutral, "neutral"),
        (theme.neutral_content, "neutral-c"),
    ];
    let status = [
        (theme.info, "info"),
        (theme.success, "success"),
        (theme.warning, "warning"),
        (theme.error, "error"),
    ];
    // Semantic tokens (the layer code should actually consume).
    let semantic = [
        (theme.surface_1(), "surface-1"),
        (theme.surface_2(), "surface-2"),
        (theme.surface_3(), "surface-3"),
        (theme.text_1(), "text-1"),
        (theme.text_2(), "text-2"),
        (theme.text_muted(), "text-muted"),
        (theme.border_subtle(), "border-subtle"),
        (theme.border_strong(), "border-strong"),
        (theme.brand(), "brand"),
        (theme.focus_ring(), "focus-ring"),
        (theme.selected_tint(), "selected-tint"),
        (theme.hover_tint(), "hover-tint"),
    ];

    for (title, set) in [
        ("Base", base.as_slice()),
        ("Brand & Neutral", brand.as_slice()),
        ("Status", status.as_slice()),
        ("Semantic", semantic.as_slice()),
    ] {
        let chips: Vec<Entity> = set
            .iter()
            .map(|(c, name)| swatch(commands, theme, *c, name, &hex(*c)))
            .collect();
        let g = widgets::group(commands, theme, title, &chips);
        commands.entity(content).add_children(&[g]);
    }
}

fn typography(commands: &mut Commands, content: Entity, theme: &Theme) {
    let scale_steps = [
        (scale::font_size::F00, "F00 · 12.8"),
        (scale::font_size::F0, "F0 · 16 (body)"),
        (scale::font_size::F1, "F1 · 20"),
        (scale::font_size::F2, "F2 · 25"),
        (scale::font_size::F3, "F3 · 31"),
        (scale::font_size::F4, "F4 · 39"),
        (scale::font_size::F5, "F5 · 49"),
        (scale::font_size::F6, "F6 · 61"),
    ];
    let stack = column(commands, scale::space::S);
    for (size, name) in scale_steps {
        let line = row(commands, scale::space::M);
        let tag = commands
            .spawn((
                Node {
                    width: Val::Px(120.0),
                    ..default()
                },
                Text::new(name),
                TextFont {
                    font_size: scale::font_size::F00,
                    ..default()
                },
                TextColor(theme.text_muted()),
            ))
            .id();
        let sample = label(commands, "Arenic", size, theme.text_1());
        commands.entity(line).add_children(&[tag, sample]);
        commands.entity(stack).add_children(&[line]);
    }

    let note = label(
        commands,
        "Family: Archivo (sans/display) · weights 100\u{2013}900 · scale ratio \u{2248}1.25",
        scale::font_size::F00,
        theme.text_muted(),
    );
    commands.entity(stack).add_children(&[note]);
    commands.entity(content).add_children(&[stack]);
}

fn spacing(commands: &mut Commands, content: Entity, theme: &Theme) {
    let steps = [
        (scale::space::XS3, "3xs · 5"),
        (scale::space::XS2, "2xs · 10"),
        (scale::space::XS, "xs · 15"),
        (scale::space::S, "s · 20"),
        (scale::space::M, "m · 30"),
        (scale::space::L, "l · 40"),
        (scale::space::XL, "xl · 60"),
        (scale::space::XL2, "2xl · 80"),
        (scale::space::XL3, "3xl · 120"),
    ];
    let stack = column(commands, scale::space::XS);
    for (px, name) in steps {
        let line = row(commands, scale::space::S);
        let tag = commands
            .spawn((
                Node {
                    width: Val::Px(90.0),
                    ..default()
                },
                Text::new(name),
                TextFont {
                    font_size: scale::font_size::F00,
                    ..default()
                },
                TextColor(theme.text_muted()),
            ))
            .id();
        let bar = commands
            .spawn((
                Node {
                    width: Val::Px(px),
                    height: Val::Px(16.0),
                    border_radius: BorderRadius::all(Val::Px(scale::radius::XS2)),
                    ..default()
                },
                BackgroundColor(theme.brand()),
            ))
            .id();
        commands.entity(line).add_children(&[tag, bar]);
        commands.entity(stack).add_children(&[line]);
    }
    commands.entity(content).add_children(&[stack]);
}

fn radii(commands: &mut Commands, content: Entity, theme: &Theme) {
    let steps = [
        (scale::radius::XS3, "3xs"),
        (scale::radius::XS2, "2xs"),
        (scale::radius::XS, "xs"),
        (scale::radius::S, "s"),
        (scale::radius::M, "m"),
        (scale::radius::L, "l"),
        (scale::radius::XL, "xl"),
        (scale::radius::XL2, "2xl"),
    ];
    let chips: Vec<Entity> = steps
        .iter()
        .map(|(r, name)| {
            let col = column(commands, scale::space::XS3);
            let chip = commands
                .spawn((
                    Node {
                        width: Val::Px(72.0),
                        height: Val::Px(72.0),
                        border: UiRect::all(Val::Px(2.0)),
                        border_radius: BorderRadius::all(Val::Px(*r)),
                        ..default()
                    },
                    BackgroundColor(theme.surface_3()),
                    BorderColor::all(theme.border_strong()),
                ))
                .id();
            let cap = label(commands, name, scale::font_size::F00, theme.text_muted());
            commands.entity(col).add_children(&[chip, cap]);
            col
        })
        .collect();
    let grid = wrap(commands, scale::space::S);
    commands.entity(grid).add_children(&chips);
    commands.entity(content).add_children(&[grid]);
}

fn elevation(commands: &mut Commands, content: Entity, theme: &Theme) {
    // Soft, theme-toned shadows (like the source's text-derived shadows).
    let soft = theme.text_1().with_alpha(0.18);
    let levels = [
        (4.0, 8.0, "shadow-1"),
        (8.0, 16.0, "shadow-2"),
        (12.0, 28.0, "shadow-3"),
        (16.0, 40.0, "shadow-4"),
        (24.0, 56.0, "shadow-5"),
    ];
    let grid = wrap(commands, scale::space::L);
    for (y, blur, name) in levels {
        let col = column(commands, scale::space::XS3);
        let card = commands
            .spawn((
                Node {
                    width: Val::Px(110.0),
                    height: Val::Px(72.0),
                    border_radius: BorderRadius::all(Val::Px(theme.radius_box)),
                    ..default()
                },
                BackgroundColor(theme.surface_2()),
                BoxShadow::new(
                    soft,
                    Val::Px(0.0),
                    Val::Px(y),
                    Val::Px(-y * 0.4),
                    Val::Px(blur),
                ),
            ))
            .id();
        let cap = label(commands, name, scale::font_size::F00, theme.text_muted());
        commands.entity(col).add_children(&[card, cap]);
        commands.entity(grid).add_children(&[col]);
    }

    // Neobrutalist hard shadow — the source's signature flat offset.
    let nb_col = column(commands, scale::space::XS3);
    let nb = commands
        .spawn((
            Node {
                width: Val::Px(110.0),
                height: Val::Px(72.0),
                border: UiRect::all(Val::Px(2.0)),
                border_radius: BorderRadius::all(Val::Px(scale::radius::XS)),
                ..default()
            },
            BackgroundColor(theme.surface_1()),
            BorderColor::all(theme.text_1()),
            BoxShadow::new(
                theme.text_1(),
                Val::Px(5.0),
                Val::Px(5.0),
                Val::Px(0.0),
                Val::Px(0.0),
            ),
        ))
        .id();
    let nb_cap = label(commands, "hard 5,5,0", scale::font_size::F00, theme.text_muted());
    commands.entity(nb_col).add_children(&[nb, nb_cap]);
    commands.entity(grid).add_children(&[nb_col]);

    commands.entity(content).add_children(&[grid]);
}

fn components(commands: &mut Commands, content: Entity, theme: &Theme) {
    // Buttons row (neobrutalist: 2px text-1 border + hard shadow). Each is
    // interactive — hover lifts, active presses, focus shows a ring — via the
    // shared `Interactive` driver, themed from the active palette.
    let buttons = row(commands, scale::space::S);
    let btn = |commands: &mut Commands, bg: Color, fg: Color, text: &str| -> Entity {
        let (hover, active) = theme.interactions(bg);
        let b = commands
            .spawn((
                Button,
                Node {
                    padding: UiRect::axes(Val::Px(scale::space::S), Val::Px(scale::space::XS2)),
                    border: UiRect::all(Val::Px(2.0)),
                    border_radius: BorderRadius::all(Val::Px(theme.radius_field)),
                    align_items: AlignItems::Center,
                    ..default()
                },
                BackgroundColor(bg),
                BorderColor::all(theme.text_1()),
                BoxShadow::new(
                    theme.text_1(),
                    Val::Px(3.0),
                    Val::Px(3.0),
                    Val::Px(0.0),
                    Val::Px(0.0),
                ),
                UiTransform::default(),
                Interactive::flat(bg, hover, active, theme.focus_ring()).with_shadow(theme.text_1(), 3.0),
                hidden_outline(),
            ))
            .id();
        let t = label(commands, text, scale::font_size::F0, fg);
        commands.entity(b).add_children(&[t]);
        b
    };
    let primary = btn(commands, theme.primary, theme.primary_content, "Primary");
    let secondary = btn(commands, theme.secondary, theme.secondary_content, "Secondary");
    let soft = btn(commands, theme.primary_soft(), theme.primary, "Soft");
    commands.entity(buttons).add_children(&[primary, secondary, soft]);

    // Status badges (soft fill + status-coloured text, pill radius).
    let badges = row(commands, scale::space::XS);
    let badge = |commands: &mut Commands, bg: Color, fg: Color, text: &str| -> Entity {
        let b = commands
            .spawn((
                Node {
                    padding: UiRect::axes(Val::Px(scale::space::XS), Val::Px(scale::space::XS3)),
                    border_radius: BorderRadius::all(Val::Px(scale::radius::PILL)),
                    ..default()
                },
                BackgroundColor(bg),
            ))
            .id();
        let t = label(commands, text, scale::font_size::F00, fg);
        commands.entity(b).add_children(&[t]);
        b
    };
    let b_info = badge(commands, theme.info_soft(), theme.info, "info");
    let b_ok = badge(commands, theme.success_soft(), theme.success, "success");
    let b_warn = badge(commands, theme.warning_soft(), theme.warning, "warning");
    let b_err = badge(commands, theme.error_soft(), theme.danger(), "error");
    commands.entity(badges).add_children(&[b_info, b_ok, b_warn, b_err]);

    // A card using surface + border + radius-box tokens.
    let card = commands
        .spawn((
            Node {
                width: Val::Px(320.0),
                flex_direction: FlexDirection::Column,
                row_gap: Val::Px(scale::space::XS2),
                padding: UiRect::all(Val::Px(scale::space::S)),
                border: UiRect::all(Val::Px(theme.border.max(1.0))),
                border_radius: BorderRadius::all(Val::Px(theme.radius_box)),
                ..default()
            },
            BackgroundColor(theme.surface_2()),
            BorderColor::all(theme.border_subtle()),
        ))
        .id();
    let card_title = label(commands, "Card", scale::font_size::F2, theme.text_1());
    let card_body = label(
        commands,
        "Surfaces, borders and radii come straight from the active theme's tokens.",
        scale::font_size::F0,
        theme.text_2(),
    );
    commands.entity(card).add_children(&[card_title, card_body]);

    commands
        .entity(content)
        .add_children(&[buttons, badges, card]);
}

/// Renders a 3D story: a caption plus a framed 16:9 viewport showing the live
/// render of the shared top-down [`crate::stage`]. Every 3D story reuses this —
/// the content on the board is what differs, and that lives in the stage, not
/// here — so adding a story duplicates **none** of the viewport scaffolding.
fn stage_story(
    commands: &mut Commands,
    content: Entity,
    theme: &Theme,
    stage: &Handle<Image>,
    note_text: &str,
) {
    let note = label(commands, note_text, scale::font_size::F0, theme.text_2());

    // Framed 16:9 viewport showing the live render of the top-down stage.
    let (w, h) = (768.0, 432.0);
    let frame = commands
        .spawn((
            Node {
                width: Val::Px(w),
                height: Val::Px(h),
                border: UiRect::all(Val::Px(theme.border.max(1.0))),
                border_radius: BorderRadius::all(Val::Px(theme.radius_box)),
                overflow: Overflow::clip(),
                ..default()
            },
            BorderColor::all(theme.border_subtle()),
        ))
        .id();
    let view = commands
        .spawn((
            ImageNode::new(stage.clone()),
            Node {
                width: Val::Px(w),
                height: Val::Px(h),
                ..default()
            },
        ))
        .id();
    commands.entity(frame).add_children(&[view]);

    commands.entity(content).add_children(&[note, frame]);
}
