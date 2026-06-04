// Package arenas is the typed model of Arenic's nine boss arenas — the canonical
// spec after the 2026-06-04 remap.
//
// THE GRID. Arenas keep their fixed 3×3 positions; the CLASSES were reassigned,
// and each class's boss, theme, atmosphere and props travel with the class:
//
//	[0] Hunter·Labyrinth   [1] Guildmaster·Guild House  [2] Cardinal·Sanctum
//	[3] Forager·Mountain    [4] Warrior·Bastion          [5] Thief·Pawnshop
//	[6] Alchemist·Crucible  [7] Merchant·Casino          [8] Bard·Gala
//
// Guild House is the Guildmaster's HOME (a safe hearth, not a real boss — an
// abstract pyramid). The old "Mixed/Gala constellation" capstone is retired
// (Gala is the Bard's arena now).
//
// THE ATMOSPHERE (shared by all nine; see assets/shaders/arena_fog.wgsl +
// foreground.rs + stage.rs). Four depth layers create dynamic layering:
//   - a SKYBOX plane far behind the floor (opaque backdrop, per-arena clouds),
//   - a SKY-SWARM of emissive motes UNDER the floor (z in [-2.3,-0.5]) — they go
//     around and under the ground but never THROUGH it, glowing up through the glass,
//   - the arena FLOOR as "liquid glass" (a translucent AlphaMode::Blend
//     StandardMaterial, perceptual_roughness 0.4: the skybox + under-floor swarm
//     read through it, and being a LIT sheet it still RECEIVES the bosses' shadows.
//     NB: screen-space specular transmission is unreliable on the HDR
//     render-to-texture stage camera, so the glass is alpha blend, not refraction), and
//   - a FOREGROUND plane between camera and floor (edge-framed, subtle haze).
//
// The skybox and foreground are TWO VOICES: the skybox a continuous FIELD effect,
// the foreground a discrete NEAR effect — a counterpoint contrasting on type,
// direction, register and tempo (see Atmosphere). Both re-tone per theme.
package arenas

// ColorRole is one palette colour plus the job it does in an arena.
type ColorRole struct {
	Hex      string // sRGB hex
	Role     string // what it lights
	Emissive bool   // HDR-emissive (bloom) vs reflected
}

// Theme is the arena's matched colour identity. Each class's theme travels with
// the class (Forager→Forest and Guildmaster→Coffee are the post-remap changes;
// the bespoke neon "Gala" theme is retired to EXTRA).
type Theme struct {
	Name    string
	Source  string // "existing" | "daisyUI" | "Ghostty"
	MassHex string // the dark MASS hex (80–90% of every element)
	Accents []ColorRole
	Family  string
	Mood    string
}

// CloudStyle is the per-arena shader-style branch in arena_fog.wgsl.
type CloudStyle string

const (
	Banded  CloudStyle = "banded"  // soft wide drifting bands
	Billows CloudStyle = "billows" // rolling fbm smog, two layers
	Lid     CloudStyle = "lid"     // ash pressing down (denser at top)
	Updraft CloudStyle = "updraft" // ash/spores rising (denser at bottom)
	Spiral  CloudStyle = "spiral"  // lazy swirling smoke
	Embers  CloudStyle = "embers"  // soft glowing motes rising
	Sweep   CloudStyle = "sweep"   // wide soft searchlight band sweeping
	Pulse   CloudStyle = "pulse"   // whole haze brightening on a beat
	Hearth  CloudStyle = "hearth"  // gently breathing warm haze (home)
	Grain   CloudStyle = "grain"   // fine drifting dust (a foreground voice)
	Streaks CloudStyle = "streaks" // elongated streaks along the drift (foreground)
	Ripple  CloudStyle = "ripple"  // concentric ripples from centre (foreground)
)

// Atmosphere is the per-arena TWO-VOICE cloud overlay. Harmony framework: the
// SKYBOX is a continuous FIELD effect; the FOREGROUND is a discrete/structured
// NEAR effect — a deliberate counterpoint contrasting on TYPE (field vs near),
// DIRECTION (perpendicular/opposite drift), REGISTER (the foreground is finer)
// and TEMPO. Across arenas both the skybox set and the foreground set stay
// distinct. The floor between them is the shared liquid-glass sheet.
type Atmosphere struct {
	SkyStyle  CloudStyle // the skybox FIELD effect
	SkyMotion string
	FgStyle   CloudStyle // the foreground NEAR effect (the counterpoint)
	FgMotion  string
	Harmony   string // how the two voices contrast
}

// Prop is one ambient flora/fauna prop on the tiles — a low-poly placeholder
// primitive (real meshes later), spawned as the FloraFauna layer in stage.rs.
type Prop struct {
	Name      string
	Kind      string // "flora" | "fauna"
	Primitive string // cuboid | sphere | cylinder | cone | capsule
	Tint      string // theme token: primary | secondary | accent | warning | surface | muted
	Note      string
}

// LevelDetails is the playfield floor's embedded, on-theme signature (the inlay
// or sigil seen through the liquid-glass floor).
type LevelDetails struct {
	Surface   string
	Signature string
}

// Distinctiveness records how this arena is held apart from its nearest neighbour.
type Distinctiveness struct {
	NearestArena int
	OnAxis       string
	Separator    string
	Signature    string
}

// Arena is one complete arena: a unique tuple after the remap.
type Arena struct {
	Index      int
	Name       string
	Class      string
	Boss       string
	BossShape  string
	Light      string // the boss's signature hollow-light behaviour
	Theme      Theme
	Atmosphere Atmosphere
	Level      LevelDetails
	Props      []Prop
	Distinct   Distinctiveness
}

var Arenas = [9]Arena{
	{
		Index: 0, Name: "Labyrinth", Class: "Hunter", Boss: "Hollow Obelisk",
		BossShape: "tall four-sided obelisk", Light: "ObeliskBlade",
		Theme: Theme{
			Name: "Tokyo Night", Source: "Ghostty", MassHex: "#1a1b26", Family: "cool-indigo",
			Accents: []ColorRole{{Hex: "#7dcfff", Role: "moonbeam highlight", Emissive: true}, {Hex: "#7aa2f7", Role: "blue ambient", Emissive: false}},
			Mood:    "Moonlit stone maze, one cold blue sightline sweeping the dark",
		},
		Atmosphere: Atmosphere{SkyStyle: Banded, SkyMotion: "broad cyan bands drifting sideways", FgStyle: Streaks, FgMotion: "fine cyan streaks falling, faster", Harmony: "horizontal field vs vertical near-streaks — perpendicular, finer, faster"},
		Level:      LevelDetails{Surface: "wet near-black flagstone (through the glass floor)", Signature: "one frozen cyan sightline groove"},
		Props: []Prop{
			{Name: "Lanternthorn Hedge", Kind: "flora", Primitive: "cuboid", Tint: "surface", Note: "low maze-wall block"},
			{Name: "Sightline Reed", Kind: "flora", Primitive: "cylinder", Tint: "primary", Note: "glowing cyan stalk"},
			{Name: "Lunar Stalker Owl", Kind: "fauna", Primitive: "capsule", Tint: "muted", Note: "perched dim watcher"},
		},
		Distinct: Distinctiveness{NearestArena: 5, OnAxis: "theme", Separator: "Hunter cyan is a passive moonbeam point; Thief cyan is an active sweep + warm torch", Signature: "deep-blue maze with a slow banded moon-haze"},
	},
	{
		Index: 1, Name: "Guild House", Class: "Guildmaster", Boss: "Home (abstract pyramid)",
		BossShape: "smooth square pyramid (reuses boss_a)", Light: "none — the safe home, not a boss",
		Theme: Theme{
			Name: "Coffee", Source: "daisyUI", MassHex: "#261b25", Family: "warm-hearth",
			Accents: []ColorRole{{Hex: "#db924c", Role: "ember/hearth", Emissive: true}, {Hex: "#c59f61", Role: "clay (desaturated)", Emissive: false}},
			Mood:    "The guild's warm, safe home — the one bright arena",
		},
		Atmosphere: Atmosphere{SkyStyle: Hearth, SkyMotion: "warm haze breathing in place", FgStyle: Embers, FgMotion: "warm motes rising", Harmony: "still breathing field vs rising discrete motes — type + direction"},
		Level:      LevelDetails{Surface: "warm wooden hall floor", Signature: "a hearth-ring inlay"},
		Props: []Prop{
			{Name: "Hearth", Kind: "flora", Primitive: "cuboid", Tint: "primary", Note: "warm glowing block"},
			{Name: "Barrel", Kind: "flora", Primitive: "cylinder", Tint: "surface", Note: "dark store barrel"},
			{Name: "Lantern Post", Kind: "fauna", Primitive: "capsule", Tint: "warning", Note: "warm lantern post"},
		},
		Distinct: Distinctiveness{NearestArena: 3, OnAxis: "boss", Separator: "home is a SMOOTH solid pyramid with no light; Mountain's ziggurat is tiered + ZigguratGrow", Signature: "warm hearth home, the only safe arena"},
	},
	{
		Index: 2, Name: "Sanctum", Class: "Cardinal", Boss: "Torus Halo",
		BossShape: "thin even ring, large hole", Light: "HaloFill",
		Theme: Theme{
			Name: "Luxury", Source: "existing", MassHex: "#0a0a08", Family: "warm-gold",
			Accents: []ColorRole{{Hex: "#dca54d", Role: "holy gold", Emissive: true}, {Hex: "#e2d4b8", Role: "champagne", Emissive: false}},
			Mood:    "Black cathedral kneeling around one gold ring",
		},
		Atmosphere: Atmosphere{SkyStyle: Billows, SkyMotion: "gold incense billows rolling", FgStyle: Grain, FgMotion: "fine gilt grain sifting down", Harmony: "large rolling field vs fine falling grain — opposite vertical + register"},
		Level:      LevelDetails{Surface: "polished black basalt", Signature: "a concentric gold-inlay ring beneath the halo"},
		Props: []Prop{
			{Name: "Censer-Lily", Kind: "flora", Primitive: "cone", Tint: "primary", Note: "gold trumpet bloom"},
			{Name: "Reliquary Reed", Kind: "flora", Primitive: "cylinder", Tint: "surface", Note: "dark relic column"},
			{Name: "Choir Ember", Kind: "fauna", Primitive: "sphere", Tint: "warning", Note: "floating gold mote"},
		},
		Distinct: Distinctiveness{NearestArena: 7, OnAxis: "theme", Separator: "Sanctum is monochrome reverent gold; Casino is gold-luck PLUS cold-odds cyan", Signature: "holy gold on near-black with faint rising embers"},
	},
	{
		Index: 3, Name: "Mountain", Class: "Forager", Boss: "Stepped Pyramid",
		BossShape: "≥3 concentric nested-square tiers", Light: "ZigguratGrow",
		Theme: Theme{
			Name: "Forest", Source: "existing", MassHex: "#06150c", Family: "woodland-green",
			Accents: []ColorRole{{Hex: "#009b53", Role: "green growth", Emissive: true}, {Hex: "#dea143", Role: "amber", Emissive: false}},
			Mood:    "The mountain forager — terrain, geology, green growth",
		},
		Atmosphere: Atmosphere{SkyStyle: Updraft, SkyMotion: "green spores rising", FgStyle: Grain, FgMotion: "wind-grain drifting sideways", Harmony: "vertical rise vs horizontal grain — perpendicular"},
		Level:      LevelDetails{Surface: "earthen plum-basalt", Signature: "an off-axis geological fault-vein"},
		Props: []Prop{
			{Name: "Emberfig Cluster", Kind: "flora", Primitive: "sphere", Tint: "surface", Note: "dark succulent dome"},
			{Name: "Cinder Pillbug", Kind: "fauna", Primitive: "sphere", Tint: "warning", Note: "glowing rolled critter"},
			{Name: "Slag-Reed Tuft", Kind: "flora", Primitive: "cone", Tint: "accent", Note: "mineral spike"},
		},
		Distinct: Distinctiveness{NearestArena: 4, OnAxis: "theme", Separator: "Mountain is GREEN/earthy (Forager); Bastion is ORANGE forge (Warrior) — re-themed to clear the old ember twins", Signature: "green mountain updraft over a fault-vein floor"},
	},
	{
		Index: 4, Name: "Bastion", Class: "Warrior", Boss: "Hexagonal Prism",
		BossShape: "heavy hexagonal block", Light: "PrismBlock",
		Theme: Theme{
			Name: "Gruvbox Dark", Source: "Ghostty", MassHex: "#282828", Family: "forge-orange",
			Accents: []ColorRole{{Hex: "#fe8019", Role: "molten orange", Emissive: true}, {Hex: "#fabd2f", Role: "ember gold", Emissive: true}},
			Mood:    "The warrior's forge-fortress — armour, defence, furnace",
		},
		Atmosphere: Atmosphere{SkyStyle: Lid, SkyMotion: "orange ash pressing down", FgStyle: Embers, FgMotion: "sparks rising fast", Harmony: "descending field vs rising sparks — opposite direction + tempo"},
		Level:      LevelDetails{Surface: "quenched basalt", Signature: "a centred anvil-hexagon sigil that pulses on bloom"},
		Props: []Prop{
			{Name: "Slag Spike", Kind: "flora", Primitive: "cone", Tint: "surface", Note: "dark hex shard"},
			{Name: "Cinder Beetle", Kind: "fauna", Primitive: "sphere", Tint: "primary", Note: "glowing scarab"},
			{Name: "Forge-Lichen", Kind: "flora", Primitive: "cylinder", Tint: "warning", Note: "ember crust disc"},
		},
		Distinct: Distinctiveness{NearestArena: 3, OnAxis: "theme", Separator: "Bastion is high-chroma forge-orange + a pressing lid; Mountain is green + a rising updraft", Signature: "soot-grey fortress under a molten lid"},
	},
	{
		Index: 5, Name: "Pawnshop", Class: "Thief", Boss: "Triangular Prism",
		BossShape: "triangular wedge", Light: "WedgeBeam",
		Theme: Theme{
			Name: "Ayu Dark", Source: "Ghostty", MassHex: "#0a0e14", Family: "watch-cyan",
			Accents: []ColorRole{{Hex: "#39bae6", Role: "watch-cyan", Emissive: true}, {Hex: "#ff8f40", Role: "minority warm", Emissive: true}},
			Mood:    "The thief's fencing den — stealth, shadow, stolen goods",
		},
		Atmosphere: Atmosphere{SkyStyle: Banded, SkyMotion: "cyan bands, slow vertical drift", FgStyle: Sweep, FgMotion: "a cyan watch-band sweeping across", Harmony: "vertical bands vs horizontal sweep — perpendicular"},
		Level:      LevelDetails{Surface: "worn black slate", Signature: "a rationed warm torch-seam crack"},
		Props: []Prop{
			{Name: "Watcher's Lantern-Cap", Kind: "flora", Primitive: "sphere", Tint: "primary", Note: "cyan hollow cap"},
			{Name: "Sentry Beetle", Kind: "fauna", Primitive: "cone", Tint: "surface", Note: "dark prism shell"},
			{Name: "Lockpick Reed", Kind: "flora", Primitive: "capsule", Tint: "secondary", Note: "amber hooked stalk"},
		},
		Distinct: Distinctiveness{NearestArena: 0, OnAxis: "theme", Separator: "Thief cyan is an active wide sweep + a warm torch; Hunter cyan is a static moonbeam point", Signature: "near-black den with a slow wide cyan sweep"},
	},
	{
		Index: 6, Name: "Crucible", Class: "Alchemist", Boss: "Truncated Cone",
		BossShape: "thick-walled cone, small mouth", Light: "CauldronRise",
		Theme: Theme{
			Name: "Abyss", Source: "daisyUI", MassHex: "#001e29", Family: "toxic-lime",
			Accents: []ColorRole{{Hex: "#bdff00", Role: "acid bioluminescence", Emissive: true}, {Hex: "#003843", Role: "secondary mass", Emissive: false}},
			Mood:    "The alchemist's vessel — transformation, toxic pools",
		},
		Atmosphere: Atmosphere{SkyStyle: Billows, SkyMotion: "lime smog rolling, two layers", FgStyle: Ripple, FgMotion: "lime bubble-ripples popping", Harmony: "linear rolling field vs radial ripples — type + pattern"},
		Level:      LevelDetails{Surface: "submerged flagstone under black-green brine", Signature: "a central alchemist sigil pulsing lime"},
		Props: []Prop{
			{Name: "Cauldron Lily", Kind: "flora", Primitive: "cylinder", Tint: "primary", Note: "lime pool pad"},
			{Name: "Bilesnail", Kind: "fauna", Primitive: "sphere", Tint: "surface", Note: "dark coiled shell"},
			{Name: "Distiller's Reed", Kind: "flora", Primitive: "cylinder", Tint: "secondary", Note: "teal pipette tube"},
		},
		Distinct: Distinctiveness{NearestArena: 3, OnAxis: "atmosphere", Separator: "Crucible is acid-lime billows; Mountain is green updraft — lime vs woodland green, rolling vs rising", Signature: "acid-lime vessel over a brine sigil"},
	},
	{
		Index: 7, Name: "Casino", Class: "Merchant", Boss: "Hollow Icosphere",
		BossShape: "low-faceted geode", Light: "GeodeShimmer",
		Theme: Theme{
			Name: "Rosé Pine", Source: "Ghostty", MassHex: "#191724", Family: "gold-plum",
			Accents: []ColorRole{{Hex: "#f6c177", Role: "gold-luck", Emissive: true}, {Hex: "#9ccfd8", Role: "cold-odds cyan", Emissive: true}},
			Mood:    "The merchant's floor — gambling, luck, risk-reward",
		},
		Atmosphere: Atmosphere{SkyStyle: Spiral, SkyMotion: "gold-plum smoke swirling", FgStyle: Streaks, FgMotion: "coin-streaks tumbling diagonally", Harmony: "swirl field vs linear streaks — type + direction"},
		Level:      LevelDetails{Surface: "felt-dark gambling floor", Signature: "async gold/cyan tally sparkle (no beat)"},
		Props: []Prop{
			{Name: "Coinpurse Lily", Kind: "flora", Primitive: "sphere", Tint: "warning", Note: "gold coin pod"},
			{Name: "Tally Reed", Kind: "flora", Primitive: "cylinder", Tint: "surface", Note: "dark tally rods"},
			{Name: "Diecrawler", Kind: "fauna", Primitive: "cuboid", Tint: "accent", Note: "cyan d6 beetle"},
		},
		Distinct: Distinctiveness{NearestArena: 2, OnAxis: "theme", Separator: "Casino keeps a cold-odds cyan alongside gold; Sanctum is monochrome reverent gold", Signature: "gold-plum casino with a lazy spiral haze"},
	},
	{
		Index: 8, Name: "Gala", Class: "Bard", Boss: "Capsule Resonator",
		BossShape: "ribbed capsule + filament", Light: "CapsulePulse",
		Theme: Theme{
			Name: "Synthwave", Source: "existing", MassHex: "#2b1840", Family: "synth-pink",
			Accents: []ColorRole{{Hex: "#ff4da6", Role: "on-beat pink", Emissive: true}, {Hex: "#a0e0ff", Role: "off-beat cyan", Emissive: true}},
			Mood:    "The bard's festival — rhythm, performance, neon",
		},
		Atmosphere: Atmosphere{SkyStyle: Billows, SkyMotion: "violet haze drifting", FgStyle: Pulse, FgMotion: "pink/cyan throb on the beat", Harmony: "steady drifting field vs rhythmic pulse — tempo/rhythm"},
		Level:      LevelDetails{Surface: "rhythmic ledger floor (placeholder; festival re-theme pending)", Signature: "seams brightening on the beat"},
		Props: []Prop{
			{Name: "Pawn-Tag Lily", Kind: "flora", Primitive: "cuboid", Tint: "primary", Note: "pink tag card (placeholder — re-theme for festival)"},
			{Name: "Coil-Vine", Kind: "flora", Primitive: "cylinder", Tint: "secondary", Note: "cyan cable coil"},
			{Name: "Echo Koi", Kind: "fauna", Primitive: "capsule", Tint: "surface", Note: "dark gliding fish"},
		},
		Distinct: Distinctiveness{NearestArena: 0, OnAxis: "atmosphere", Separator: "Gala pulses on a beat (pink/cyan); no other arena globally pulses", Signature: "neon festival capsule pulsing on the beat"},
	},
}

// --- Uniqueness audit (re-validated for the post-remap grid) ----------------

type AxisDistinctness struct {
	Axis    string
	MinDist float64 // smallest pairwise distance (0..1)
	Verdict string
}

type WatchPair struct {
	A, B       int
	Axis       string
	Closeness  float64
	Resolution string
}

type UniquenessAudit struct {
	Axes       []AxisDistinctness
	WatchPairs []WatchPair
	Summary    string
}

var Audit = UniquenessAudit{
	Axes: []AxisDistinctness{
		{Axis: "theme", MinDist: 0.42, Verdict: "GREEN with separators: the warm middle column (Coffee/Gruvbox + Luxury/Rosé Pine) is the tightest — split on chroma + the cold-odds cyan; Forest/Abyss split woodland-green vs acid-lime"},
		{Axis: "atmosphere", MinDist: 0.50, Verdict: "GREEN: each arena pairs a distinct SKYBOX field voice with a contrasting FOREGROUND near voice (the harmony framework — field vs near, perpendicular/opposite drift, finer register, different tempo); both sets stay distinct across arenas via colour + style + direction"},
		{Axis: "boss", MinDist: 0.40, Verdict: "GREEN with separators: home pyramid vs Mountain ziggurat (smooth-solid vs tiered+light), and the round-hollow trio torus/cone/icosphere (thin-ring vs thick-wall vs faceted)"},
		{Axis: "props", MinDist: 0.45, Verdict: "GREEN: 3 class-keyed props per arena, distinct names/shapes/tints (placeholders; some, e.g. Bard's Pawn-Tag, pending festival re-theme)"},
		{Axis: "level", MinDist: 0.40, Verdict: "GREEN: per-arena floor signatures (sightline groove / hearth ring / gold ring / fault-vein / anvil sigil / torch-seam / alchemist sigil / tally sparkle / beat seams)"},
	},
	WatchPairs: []WatchPair{
		{A: 1, B: 4, Axis: "theme", Closeness: 0.55, Resolution: "Coffee = desaturated hearth-brown (low chroma); Gruvbox = high-chroma forge-orange. Keep the chroma + plum-vs-soot mass split"},
		{A: 2, B: 7, Axis: "theme", Closeness: 0.50, Resolution: "Luxury = monochrome reverent gold; Rosé Pine = gold + a cold-odds cyan. Keep the cyan in Casino"},
		{A: 1, B: 3, Axis: "boss", Closeness: 0.58, Resolution: "Guild House home = a single smooth solid pyramid with NO light behaviour; Mountain = ≥3 stepped tiers + ZigguratGrow"},
		{A: 2, B: 6, Axis: "boss", Closeness: 0.52, Resolution: "Torus = thin even ring + orbiting HaloFill; Cone = thick wall + small mouth + vertical CauldronRise"},
	},
	Summary: "The post-remap grid is distinct yet cohesive. Cohesion is structural: all nine share the four-layer atmosphere (opaque skybox → under-floor sky-swarm → liquid-glass floor → foreground haze), the hollow-light/HDR+bloom law, the dark-MASS + rationed-accent palette, and class-keyed prop families. The remap also resolved the old Mountain/Crucible ember-twin clash (Mountain is now Forager/Forest green, Crucible is Alchemist/Abyss lime). The two new watch-pairs — the warm middle column (theme) and home-pyramid vs ziggurat (boss) — clear the 0.40 gate via the listed separators.",
}
