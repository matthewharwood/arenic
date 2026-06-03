//! Design tokens, ported from the engmanager.xyz CSS theming system.
//!
//! The architecture mirrors the source's three tiers:
//!
//! 1. **Primitives** — a per-theme [`Theme`] palette of raw oklch colours
//!    (`base_*`, `primary`, status colours …), plus the structural knobs
//!    `border` / `depth` / `radius_*`. See [`palettes`] for the 9 themes.
//! 2. **Semantic tokens** — surfaces, text, borders, brand, focus, interaction
//!    tints — derived from the primitives by the methods on [`Theme`]. Code
//!    consumes these, never the raw primitives, so swapping the palette
//!    re-themes everything.
//! 3. **Scales** — theme-independent type, space, radius and weight steps in
//!    [`scale`]. The CSS uses fluid `clamp()`; we take each range's upper bound
//!    as a crisp pixel value for the desktop UI.
//!
//! Switch themes by swapping the [`ActiveTheme`] resource's [`ThemeId`].

pub mod palettes;
pub mod scale;

use bevy::color::Alpha;
use bevy::prelude::*;

/// A complete colour palette plus the per-theme structural knobs. Fields are the
/// raw *primitive* tokens (oklch); use the methods for *semantic* tokens.
#[derive(Debug, Clone, Copy)]
pub struct Theme {
    pub base_100: Color,
    pub base_200: Color,
    pub base_300: Color,
    pub base_content: Color,
    pub primary: Color,
    pub primary_content: Color,
    pub secondary: Color,
    pub secondary_content: Color,
    pub accent: Color,
    pub accent_content: Color,
    pub neutral: Color,
    pub neutral_content: Color,
    pub info: Color,
    pub info_content: Color,
    pub success: Color,
    pub success_content: Color,
    pub warning: Color,
    pub warning_content: Color,
    pub error: Color,
    pub error_content: Color,
    /// Base border width for this theme, px.
    pub border: f32,
    /// Whether the theme leans on soft shadows (`1.0`) or flat fills (`0.0`).
    pub depth: f32,
    /// Structural radii, px: small controls, fields/buttons, and boxes/cards.
    pub radius_selector: f32,
    pub radius_field: f32,
    pub radius_box: f32,
}

impl Theme {
    // --- Surfaces -----------------------------------------------------------
    pub fn surface_1(&self) -> Color {
        self.base_100
    }
    pub fn surface_2(&self) -> Color {
        self.base_200
    }
    pub fn surface_3(&self) -> Color {
        self.base_300
    }
    pub fn surface_elevated(&self) -> Color {
        self.base_100
    }
    pub fn surface_sunken(&self) -> Color {
        self.base_200
    }
    pub fn surface_overlay(&self) -> Color {
        self.base_content.with_alpha(0.40)
    }

    // --- Text ---------------------------------------------------------------
    pub fn text_1(&self) -> Color {
        self.base_content
    }
    pub fn text_2(&self) -> Color {
        self.base_content.with_alpha(0.78)
    }
    pub fn text_muted(&self) -> Color {
        self.base_content.with_alpha(0.55)
    }
    pub fn text_disabled(&self) -> Color {
        self.base_content.with_alpha(0.38)
    }
    pub fn text_on_brand(&self) -> Color {
        self.primary_content
    }

    // --- Links --------------------------------------------------------------
    pub fn link(&self) -> Color {
        self.primary
    }
    pub fn link_visited(&self) -> Color {
        self.secondary
    }

    // --- Borders (escalating contrast) -------------------------------------
    pub fn border_faint(&self) -> Color {
        self.base_content.with_alpha(0.06)
    }
    pub fn border_subtle(&self) -> Color {
        self.base_content.with_alpha(0.12)
    }
    pub fn border_muted(&self) -> Color {
        self.base_content.with_alpha(0.18)
    }
    pub fn border_strong(&self) -> Color {
        self.base_content.with_alpha(0.32)
    }
    pub fn border_bold(&self) -> Color {
        self.base_content.with_alpha(0.55)
    }

    // --- Brand & status -----------------------------------------------------
    pub fn brand(&self) -> Color {
        self.primary
    }
    pub fn brand_text(&self) -> Color {
        self.primary_content
    }
    pub fn danger(&self) -> Color {
        self.error
    }

    pub fn primary_soft(&self) -> Color {
        self.primary.with_alpha(0.12)
    }
    pub fn secondary_soft(&self) -> Color {
        self.secondary.with_alpha(0.12)
    }
    pub fn accent_soft(&self) -> Color {
        self.accent.with_alpha(0.12)
    }
    pub fn success_soft(&self) -> Color {
        self.success.with_alpha(0.15)
    }
    pub fn warning_soft(&self) -> Color {
        self.warning.with_alpha(0.15)
    }
    pub fn error_soft(&self) -> Color {
        self.error.with_alpha(0.15)
    }
    pub fn info_soft(&self) -> Color {
        self.info.with_alpha(0.15)
    }

    // --- Interaction --------------------------------------------------------
    pub fn focus_ring(&self) -> Color {
        self.primary.with_alpha(0.70)
    }
    pub fn hover_tint(&self) -> Color {
        self.base_content.with_alpha(0.06)
    }
    pub fn press_tint(&self) -> Color {
        self.base_content.with_alpha(0.10)
    }
    pub fn selected_tint(&self) -> Color {
        self.primary.with_alpha(0.14)
    }

    /// The hovered and pressed background for a control whose resting background
    /// is `rest`. Mirrors the source's `--hover-tint` / `--press-tint` overlays:
    /// over a transparent base they show through as a subtle wash; over a solid
    /// base they composite on top.
    pub fn interactions(&self, rest: Color) -> (Color, Color) {
        if rest.alpha() <= f32::EPSILON {
            (self.hover_tint(), self.press_tint())
        } else {
            (
                composite(rest, self.hover_tint()),
                composite(rest, self.press_tint()),
            )
        }
    }
}

/// Alpha-composites `over` on top of `base` (sRGB), keeping `base`'s opacity.
fn composite(base: Color, over: Color) -> Color {
    let b = base.to_srgba();
    let o = over.to_srgba();
    let a = o.alpha;
    Color::srgba(
        b.red * (1.0 - a) + o.red * a,
        b.green * (1.0 - a) + o.green * a,
        b.blue * (1.0 - a) + o.blue * a,
        b.alpha.max(a),
    )
}

/// The set of available themes. Selects which [`Theme`] palette is active.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum ThemeId {
    Light,
    Dark,
    Synthwave,
    Cyberpunk,
    Forest,
    Lofi,
    Dracula,
    Catppuccin,
    Luxury,
}

impl ThemeId {
    /// Every theme, in display order.
    pub const ALL: [ThemeId; 9] = [
        ThemeId::Light,
        ThemeId::Dark,
        ThemeId::Synthwave,
        ThemeId::Cyberpunk,
        ThemeId::Forest,
        ThemeId::Lofi,
        ThemeId::Dracula,
        ThemeId::Catppuccin,
        ThemeId::Luxury,
    ];

    /// Human-readable name, e.g. for a theme switcher.
    pub fn label(self) -> &'static str {
        match self {
            ThemeId::Light => "Light",
            ThemeId::Dark => "Dark",
            ThemeId::Synthwave => "Synthwave",
            ThemeId::Cyberpunk => "Cyberpunk",
            ThemeId::Forest => "Forest",
            ThemeId::Lofi => "Lo-Fi",
            ThemeId::Dracula => "Dracula",
            ThemeId::Catppuccin => "Catppuccin",
            ThemeId::Luxury => "Luxury",
        }
    }

    /// The concrete colour palette for this theme.
    pub fn palette(self) -> Theme {
        match self {
            ThemeId::Light => palettes::LIGHT,
            ThemeId::Dark => palettes::DARK,
            ThemeId::Synthwave => palettes::SYNTHWAVE,
            ThemeId::Cyberpunk => palettes::CYBERPUNK,
            ThemeId::Forest => palettes::FOREST,
            ThemeId::Lofi => palettes::LOFI,
            ThemeId::Dracula => palettes::DRACULA,
            ThemeId::Catppuccin => palettes::CATPPUCCIN,
            ThemeId::Luxury => palettes::LUXURY,
        }
    }

    /// The next theme in [`ThemeId::ALL`], wrapping around — handy for a
    /// "cycle theme" control.
    pub fn next(self) -> ThemeId {
        let i = ThemeId::ALL.iter().position(|&t| t == self).unwrap_or(0);
        ThemeId::ALL[(i + 1) % ThemeId::ALL.len()]
    }
}

/// The currently active theme. Insert it as a resource; swap `.0` to re-theme.
#[derive(Resource, Debug, Clone, Copy)]
pub struct ActiveTheme(pub ThemeId);

impl ActiveTheme {
    /// The active colour palette.
    pub fn palette(&self) -> Theme {
        self.0.palette()
    }
}

impl Default for ActiveTheme {
    fn default() -> Self {
        ActiveTheme(ThemeId::Dark)
    }
}
