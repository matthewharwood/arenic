# Design System — Overview

A small, themeable token system with three tiers. The rule that makes it work:
**components only ever read the semantic layer**, so swapping the primitive
palette re-themes everything with no component edits.

```text
Primitives          Semantic aliases              Components
──────────          ────────────────              ──────────
--color-base-100  → --surface-1                 → cards, panels
--color-base-content
   ├─ α .78        → --text-2                    → body copy
   ├─ α .55        → --text-muted                → captions
   └─ α .12        → --border-subtle             → dividers
--color-primary   → --brand / --focus-ring       → buttons, links, focus
```

## The three tiers

1. **Primitives** — a per-theme palette of raw `oklch` colours (`base-100/200/300`,
   `base-content`, `primary`, `secondary`, `accent`, `neutral`, and the status
   colours), plus structural knobs `--border`, `--depth` and the `--radius-*`
   values. Nine palettes ship: light, dark, synthwave, cyberpunk, forest, lo-fi,
   dracula, catppuccin, luxury.
2. **Semantic aliases** — surfaces, text, borders (faint → bold), brand, focus,
   and interaction tints (hover / press / selected). Most are the base colour at
   a set alpha, so they track the theme automatically.
3. **Scales** — theme-independent type, space, radius, weight and line-height
   steps. On the web these are fluid `clamp()` ranges; in the game they become
   fixed pixels (the range's upper bound).

## Using the tokens in Rust

The `arenic_game::theme` crate mirrors this exactly:

```rust
use arenic_game::theme::{ActiveTheme, ThemeId, scale};

// Pick a palette (a Resource you can swap at runtime to re-theme).
let theme = ThemeId::Dark.palette();

// Semantic tokens are methods; primitives are fields.
let card_bg     = theme.surface_2();
let body_text   = theme.text_2();
let divider     = theme.border_subtle();
let button_bg   = theme.brand();

// Scales are plain pixel constants.
let gap     = scale::space::M;        // 30.0
let heading = scale::font_size::F4;   // 39.06
let rounded = scale::radius::L;       // 16.0
```

See the live, switchable version in the storybook:

```sh
cargo run -p arenic_storybook
```
