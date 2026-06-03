# Arenic

This book is the home for Arenic's documentation. It has two halves:

- **Design System** — the reusable design tokens (colours, typography, space,
  radii, elevation, motion) that style every surface of the game and its tools.
  The tokens are ported from the [engmanager.xyz](https://github.com/matthewharwood/engmanager.xyz)
  CSS theming system and re-expressed as Rust in the `arenic_game::theme` crate,
  so the same vocabulary drives both the web reference and the Bevy UI.
- **Game** — the rulebook and the units/scale reference.

> The pages in the Design System section are themed by the tokens themselves.
> Toggle the book theme (the paintbrush, top-left) between light and dark and the
> swatches re-tone live — the same trick the tokens give the game.

## Where the tokens live

| Surface | Source of truth |
| --- | --- |
| Web reference | `engmanager.xyz` `critical.css` (`@layer fp.tokens` / `fp.themes`) |
| This book | `theme-css/arenic.css` (the CSS tokens, reused verbatim) |
| The game & storybook | `crates/arenic_game/src/theme/` (Rust) |
| Live, interactive | the **storybook** binary — `cargo run -p arenic_storybook` |

The storybook renders the same style guide as a native Bevy app, with a theme
switcher across all nine palettes.
