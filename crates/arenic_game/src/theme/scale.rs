//! Theme-independent scales: type, space, radius, weight, line-height.
//!
//! Ported from the engmanager.xyz token system. The CSS expresses these as
//! fluid `clamp(min, preferred, max)` ranges keyed to viewport width. A native
//! game window has no such reflow, so we take the **upper bound** of each range
//! (its `rem` value × 16) as a fixed pixel value — crisp at desktop sizes.
//!
//! All values are logical pixels (`f32`), ready to drop into `Val::Px(..)` or
//! `TextFont::font_size`.

/// Fluid **type scale** (px). A ~1.25 (major-third) ratio at the top of each
/// clamp range. `F0` is the body baseline (16px); `F00` is the fine-print step.
pub mod font_size {
    pub const F00: f32 = 12.8;
    pub const F0: f32 = 16.0;
    pub const F1: f32 = 20.0;
    pub const F2: f32 = 25.0;
    pub const F3: f32 = 31.25;
    pub const F4: f32 = 39.06;
    pub const F5: f32 = 48.83;
    pub const F6: f32 = 61.04;
    pub const F7: f32 = 76.29;
    pub const F8: f32 = 95.37;
}

/// **Space scale** (px) for gaps, padding and margins — a consistent rhythm
/// from `XS3` (5px) up to `XL4` (160px).
pub mod space {
    pub const XS3: f32 = 5.0;
    pub const XS2: f32 = 10.0;
    pub const XS: f32 = 15.0;
    pub const S: f32 = 20.0;
    pub const M: f32 = 30.0;
    pub const L: f32 = 40.0;
    pub const XL: f32 = 60.0;
    pub const XL2: f32 = 80.0;
    pub const XL3: f32 = 120.0;
    pub const XL4: f32 = 160.0;
}

/// **Radius scale** (px). `PILL` is an arbitrarily large value for fully-rounded
/// ends; for a circle use `Val::Percent(50.0)` instead of a px radius.
pub mod radius {
    pub const XS3: f32 = 3.0;
    pub const XS2: f32 = 4.0;
    pub const XS: f32 = 6.0;
    pub const S: f32 = 8.0;
    pub const M: f32 = 12.0;
    pub const L: f32 = 16.0;
    pub const XL: f32 = 24.0;
    pub const XL2: f32 = 32.0;
    pub const PILL: f32 = 9999.0;
}

/// **Font weights** (100–900). Bevy renders text in the loaded font's own
/// weight, so these are primarily for documentation and any future use of
/// weighted font assets / variable-font axes.
pub mod weight {
    pub const W1: u16 = 100;
    pub const W2: u16 = 200;
    pub const W3: u16 = 300;
    pub const W4: u16 = 400;
    pub const W5: u16 = 500;
    pub const W6: u16 = 600;
    pub const W7: u16 = 700;
    pub const W8: u16 = 800;
    pub const W9: u16 = 900;
}

/// **Line-height** ratios, tightening as type gets larger.
pub mod line_height {
    pub const LH00: f32 = 1.4;
    pub const LH0: f32 = 1.5;
    pub const LH1: f32 = 1.5;
    pub const LH2: f32 = 1.4;
    pub const LH3: f32 = 1.3;
    pub const LH4: f32 = 1.15;
    pub const LH5: f32 = 1.1;
    pub const LH6: f32 = 1.05;
    pub const LH7: f32 = 1.0;
}
