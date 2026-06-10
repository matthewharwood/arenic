# ARENIC: Complete Game Instructions

## Game Overview

**Arenic** is an innovative tactical strategy game where you command up to 320 heroes across 8 simultaneous 40-person raids. Master the revolutionary **Record & Replay** system to build layered ghost recordings, coordinate complex strategies, and conquer challenging boss encounters. Every action matters in this deterministic world where victory comes through precise timing, strategic positioning, and masterful orchestration of your guild's abilities.

---

## Core Game Concept

### The Innovation: Record & Replay System
The heart of Arenic is its unique recording mechanism:
- **Record** individual hero actions and movements in 2-minute cycles
- **Replay** those recordings as "ghost" characters that automatically repeat their actions
- **Layer** up to 40 ghost recordings per arena to build massive coordinated raids
- **Master** complex strategies by synchronizing multiple heroes across different timelines

### The Goal
Build a guild powerful enough to simultaneously manage 8 different arenas, each with its own boss and unique challenges. Progress through increasingly difficult tiers (Normal → Heroic → Mythic) while growing your roster through strategic gacha recruitment.

---

## Getting Started

### Your First Steps
1. **Arena Selection**: Begin in the Guild House (arena 1) with your starting hero
2. **Movement**: Use the arrow keys to move your hero one tile at a time on the grid
3. **Character Selection**: Press Tab to switch between available heroes in your current arena
4. **Basic Combat**: In combat arenas, your hero automatically attacks nearby enemies based on their abilities (the Guild House is a safe hearth with none)

### Understanding the Interface
- **Grid-Based Movement**: Each arena is a 66×31 tile battlefield
- **Multi-Arena View**: Access all 9 arenas through the navigation system
- **Character Status**: The selected hero carries a glowing selection ring; ghosts carry a blue ring
- **Boss Visibility**: Each combat arena holds one large themed relic boss with a glowing core (see Arena-Specific Boss Types); the Guild House has none

---

## Controls Reference

| Input | Action |
|-------|--------|
| **Arrow Keys** | Move selected character (tile-by-tile grid movement) |
| **Tab** | Switch to next character in current arena |
| **Shift+Tab** | Switch to previous character in current arena |
| **1-4** | Activate character abilities (when available) |
| **R** | Record key (context-sensitive): start the 3s countdown, or open a modal — *Record new / Replay previous / Cancel* (prior recording), *Record new / Cancel* (selected ghost), *Commit partial / Keep recording / Discard* (while recording) |
| **[ / ]** | Cycle through arenas (0-8, wraps around) |
| **P** | Toggle between single-arena view and all-arenas overview |
| **Esc** | Cancel an open modal / return to the title screen |
| **Enter** | Open guild house management menu |
| **Mouse** | Arena selection and UI interaction (optional — every modal is fully keyboard-driven) |
| **Space** | Interact with objects and chests |

### Modal Controls (keyboard-first)

Every recording decision is a TUI-style modal driven entirely from the keyboard —
think terminal permission prompts or Caves of Qud's dialogs. Mouse clicks work,
but are never required:

- Options lay out **horizontally** with numbered hotkeys — press **1-9** to choose one instantly
- **← / →** (or **Tab / Shift+Tab**) move the focus highlight between options
- **Enter** confirms the focused option; **Esc** always takes the safe/cancel option
- A hint line on the modal spells out exactly these keys (`←→ focus · Enter confirm · Esc cancel`)
- While a modal is open, world input (movement, abilities, R, Tab) is gated off — the
  number and arrow keys belong to the modal — and only that arena's clock pauses

---

## Input System Rules

**🎯 CRITICAL: Single Keypress Action System**

Arenic uses a **single keypress** input system - every action requires exactly one key press:

- **Movement**: Each arrow-key press moves your character exactly one tile
- **Abilities**: Each number key (1-4) press activates the ability once
- **No Held Keys**: Holding down movement keys does NOT create continuous movement
- **Intent-Based**: The recording system captures when you pressed a key, not how long you held it
- **Precision Required**: Strategic timing comes from when you choose to press keys, not how fast you can spam them

**Why This Matters for Recording:**
- No duplicate events are ever recorded (impossible with single keypress system)
- Ghost replays are perfectly deterministic and precise
- Strategic depth comes from timing your keypresses, not reaction speed
- Recordings are highly compressed and efficient

**Example:**
- ❌ **Wrong**: Hold ↑ to move multiple tiles → Only moves one tile on initial press
- ✅ **Correct**: Press ↑, ↑, ↑, ↑ to move four tiles → Each press moves one tile

---

## The Record & Replay System (Core Mechanic)

The recording system allows you to capture 2-minute sequences of character actions that replay as autonomous "ghosts" every arena cycle. Think of it as sheet music: every arena cycle is a fixed 2-minute score, and each character owns a per-arena **staff** — a stream of intent events (moves and abilities). Committing a recording **folds** that staff into the arena's single **master timeline**, which choreographs all of the arena's ghosts (up to 40) at once.

### How Recording Works

**Starting a Recording**
- Press R while controlling the selected character; only one recording can be in progress at a time
- No prior recording for this arena → the 3-second countdown starts immediately (no modal)
- A prior recording exists for this arena → a modal asks **Record new** / **Replay previous** / **Cancel**
- R on a selected ghost → a modal asks **Record new** / **Cancel** (recording anew unfolds the ghost's old events first)
- During the countdown the recording arena is held at 0:00 — the other arenas keep playing
- All input is ignored during the countdown — except **R**, which aborts it; movement and abilities register only once capture begins at 0:00
- Capture runs as the clock ticks 0:00 → 2:00; all movements (arrow keys) and abilities (1-4 keys) are captured as intent, stamped with the arena's current tick

**During Recording**
- You have exactly 120 seconds to record your strategy
- A red "REC" indicator and the arena clock show in the HUD while recording
- Movement stays live inside the arena; stepping past an edge that has an adjacent arena opens the **Like the recording?** modal instead of silently moving you (at the outer world border the step simply clamps, recorded as played)
- Tab is ignored while recording; the recording always belongs to the character who started it
- Recording captures your input intent, not character position - ensuring perfect replay regardless of physics changes

**Ending a Recording**
- At the 2-minute mark the clock pauses and a modal asks **Commit** / **Discard**
- Press R mid-recording to stop early: a modal asks **Commit partial** / **Keep recording** / **Discard**
- A partial commit is simply a shorter event list — the ghost replays it, then idles for the rest of each cycle
- On commit: the draft is cached in the character's per-arena library, folded into the arena's master timeline, the character becomes a ghost, and the arena restarts at 0:00
- On discard: the draft is thrown away and the character stays selected

**Recording Interruptions**
- Tab is ignored while recording — a recording cannot be switched away from
- Stepping past the arena edge opens the **Like the recording?** modal: **Continue recording** (abort the step) / **Cancel recording & walk out** (discard the draft, then perform the step) / **Commit** (fold the draft — you become a ghost and stay)
- Camera keys ([ / ] and P) never stop a recording — watching another arena is safe
- While any modal is open, only the recording arena's clock pauses

### Ghost Playback System

**Autonomous Replay**
- Each arena owns a single **master timeline**: every committed recording is cloned, tagged with its ghost, and folded into one tick-sorted event stream
- Ghosts don't replay individual timelines — the arena plays its master timeline and drives all of its ghosts (up to 40) from it
- Each arena maintains its own independent clock (0:00 to 2:00)
- Ghosts in off-screen arenas continue advancing — their arenas never stop ticking
- Whenever an arena (re)starts at 0:00 — natural clock wrap, recording start, commit, or replaying a stored recording — every folded ghost snaps to its recorded start tile and the score replays (breaking a ghost out does **not** restart the arena)

**Visual Indicators**
- Ghosts carry a blue ring; the selected character carries the white selection ring
- A red "REC" label and the arena clock show in the HUD while recording; a countdown digit shows during the 3-second countdown
- Cannot directly control ghosts - they follow the arena's master timeline (Tab can re-select one; press R on it to record anew, or an arrow key to break it out)

**Breaking a Ghost Out**
- Arrow keys never move a ghost directly — pressing one with a ghost selected opens the **Break out?** modal (pausing only that arena):
    - **Take control**: the ghost's events are unfolded from the master timeline, the Ghost state is removed, and the arena **resumes right where it left off** (no restart) — the character stays at its current position under free control
    - **Restart arena**: the arena restarts at 0:00 and every ghost snaps to its start — the character stays a folded ghost
    - **Cancel**: nothing changes; playback resumes
- A broken-out character keeps its cached recording: press **R** for *Record new / Replay previous / Cancel* — *Replay previous* folds it back in and restarts the arena

**Timeline Accuracy**
- Recording stores movement intent (arrow-key presses) not positions — transforms are derived on playback
- Events are stamped in ticks on the fixed 60 Hz timestep (7,200 ticks per 2-minute cycle), never in seconds
- Each recording remembers its start tile, so every cycle replays from the same origin
- Abilities trigger at exact recorded ticks
- Perfect deterministic replay every cycle

### Multi-Arena Coordination

**Independent Timers**
- Each of the 9 arenas has its own 2-minute cycle clock
- Ghosts use their parent arena's clock for playback
- Switching arenas ([ ]) doesn't affect other arena clocks
- While a modal is open, **only the selected arena's clock pauses** — the other eight keep ticking
- During a recording countdown, only the recording arena is held at 0:00

**Cross-Arena Strategy**
- Record complementary ghosts across multiple arenas
- Use the arena status panel to track ghost counts per arena
- Maximum of 40 ghosts per arena, 320 total across all arenas
- (Future) rendering may scale down visual update rates for distant arenas; the simulation always ticks every arena at the fixed 60 Hz timestep

### Ghost Replay Feature

**Arena-Specific Recordings**
- Each character keeps a per-arena library of recordings (a hashmap keyed by arena) — its "staff" of sheet music per arena
- A character is only ever a ghost in the arena it currently occupies; its other recordings wait in the library
- Re-committing a recording for an arena replaces the old one (unfold first, then fold — always idempotent)

**Travel: Leaving and Returning**
- Travel is edge-walking: stepping past an arena edge re-parents the hero into the adjacent arena at the opposite edge
- Edge-walking only applies between adjacent arenas — at the outer border of the 3×3 grid movement is clamped (the world does not wrap)
- Ghosts never edge-walk: break the ghost out first (see **Breaking a Ghost Out**) — its events leave the master timeline at break-out and that arena keeps playing without restarting — then walk it wherever you like
- When a hero edge-walks **into** an arena where it has a cached recording, a modal asks:
    - **Replay previous**: fold the stored recording back into this arena's master timeline — the hero becomes a ghost and the arena restarts
    - **Continue without**: stay a regular character (press R later to record new or replay via the R modal)
- This allows the same character to have different tactical roles in different arenas

**Strategic Applications**
- Create arena-specific strategies with the same character
- Build up recordings progressively as you learn each arena
- Reuse successful recordings when returning to farm or practice
- (Future) maintain separate recordings per difficulty tier — today the library is keyed by arena only

### Recording Best Practices

**Planning Your Recording**
- Think through the full 2-minute sequence before starting
- Consider boss attack patterns and timing windows
- Position yourself safely at the end for smooth looping
- Test ability combinations before committing

**Optimization Tips**
- The single-keypress system means recordings are already tiny — one small intent event per press
- There are no keyframes: positions are derived on playback by replaying intent from the recorded start tile
- Ability events are always preserved at full fidelity

**Common Patterns**
- **Tank Loop**: Record a warrior continuously taunting and blocking
- **Healer Rotation**: Set up heal timings to match damage spikes
- **DPS Burst**: Align multiple damage dealers for boss vulnerability phases
- **Resource Gathering**: Create forager ghosts to maintain mushroom gardens

### Advanced Recording Features

**State Management**
- Recording state machine: Idle → Countdown (3s / 180 ticks) → Recording; modals are tracked separately and gate input while open
- Exactly one recording can be in progress at a time; the global draft timeline is empty unless one is
- Character states: **Selected** (controlled, input-driven) and **Ghost** (folded into its arena's master timeline, input ignored)
- Commit transitions Selected → Ghost; "Record new" on a ghost or the break-out modal's "Take control" transitions Ghost → Selected after unfolding ("Take control" keeps the arena playing; "Record new" restarts it into a countdown)
- While a modal is open only the selected arena pauses — there is no global pause

**Performance Scaling (future, render-side only)**
- The simulation always advances every arena on the fixed 60 Hz tick — determinism is never traded away
- Rendering / visual update rates may later scale down for distant arenas (e.g. 30 FPS adjacent, 10-15 FPS distant)
- Automatic visual quality adjustment when performance drops

**Technical Details**
- Simulation runs on the fixed 60 Hz timestep: `CYCLE_TICKS = 7_200`, `COUNTDOWN_TICKS = 180`; all timeline math is in ticks, never seconds, using `strict_*`/`wrapping_*`/`%` arithmetic
- An event is intent, not position: `TimelineEvent { tick: u32, action: Action }` with `Action::Move(IVec2)` or `Action::Ability(u8)` — tiny and `Copy`
- A recording is `Recording { start, events: Arc<[TimelineEvent]> }` — the start tile anchors every replay; `Arc` makes timeline sharing cheap
- Each character carries a `RecordingLibrary` (hashmap: arena → recording); lookups by key only — the sim never iterates it
- Each arena root carries the master timeline: a tick-sorted `Vec<GhostEvent>` (event + owning ghost entity) with a playback cursor; **fold** merges a recording in, **unfold** retains everything but one ghost's events
- Commits are idempotent by construction: unfold first, then fold; the library insert replaces

---

## Character Classes & Abilities

### The 8 Character Classes
Each class brings unique tactical advantages and 4 specialized abilities:

#### **Hunter** - Ranged Precision Specialist
- **Auto Shot**: Automatically fires at closest enemy every 2.5 seconds
- **Poison Shot**: Toxic projectile with knockback and damage over time
- **Sniper**: Long-range precision shots targeting bosses
- **Trap**: Explosive area denial placement system

#### **Alchemist** - Support Through Transformation
- **Ironskin Draft**: Defensive potion granting damage reduction
- **Acid Flask**: Area denial through persistent acid pools
- **Transmute**: Resource conversion and material transformation
- **Siphon**: Life-draining channeled ability targeting allies

#### **Cardinal** - Divine Healer and Protector
- **Heal**: Smart-targeting restoration ability
- **Barrier**: Round-robin ally protection system
- **Beam**: Piercing divine damage with healing properties
- **Resurrect**: Ultimate revival and telegraph enhancement

#### **Warrior** - Frontline Tank and Protector
- **Block**: Directional projectile defense system
- **Bash**: Offensive shield strike with damage mitigation
- **Taunt**: Threat redirection and aggro management
- **Bulwark**: Frontal barrier and area denial defense

#### **Thief** - Stealth and Mobility Expert
- **Shadow Step**: Evasive teleportation with invulnerability frames
- **Smoke Screen**: Concealment and safe passage utility
- **Backstab**: Positional damage enhancement passive
- **Pickpocket**: Resource extraction and buff theft

#### **Bard** - Team Enhancement Specialist
- **Dance**: Rhythm-based offensive quick-time event
- **Helix**: Dual-mode aura providing healing or haste
- **Cleanse**: Team-wide debuff removal utility
- **Mimic**: Passive ability copying from adjacent allies

#### **Forager** - Terrain Manipulation Expert
- **Dig**: Multi-tile excavation and terrain preparation
- **Boulder**: Rolling stone offensive with resource collection
- **Border**: Projectile-deflecting earth barriers
- **Mushroom**: Healing garden creation on prepared terrain

#### **Merchant** - Economic Warfare Specialist
- **Dice**: Stackable critical chance enhancement
- **Coin Toss**: Economic risk-reward projectile system
- **Fortune**: Team luck enhancement aura
- **Vault**: Area-effect critical damage amplification

---

## Arena System & Boss Battles

### Arena Structure
- **Grid Size**: 66×31 tiles per arena (2,046 tiles × 9 arenas = 18,414 total battlefield)
- **Boss Positioning**: Each arena contains one major boss matching its class theme
- **Multi-Arena Management**: All 9 arenas run independently with separate timers
- **Scaling Difficulty**: Normal → Heroic → Mythic progression tiers

### Arena Names & Layout

Arenic features **9 distinct arenas** arranged in a 3×3 grid layout. Each arena has a unique name and thematic identity:

| Index | Arena Name | Grid Position | Theme |
|-------|------------|---------------|-------|
| **0** | **Labyrinth** | Top-Left (0,0) | **Hunter** — precision, traps, sightlines |
| **1** | **Guild House** | Top-Center (1,0) | **Guildmaster** — the guild's home & safe hearth (not a combat arena) |
| **2** | **Sanctum** | Top-Right (2,0) | **Cardinal** — divine magic |
| **3** | **Mountain** | Middle-Left (0,1) | **Forager** — terrain, geology, foraging |
| **4** | **Bastion** | Middle-Center (1,1) | **Warrior** — strength & defense (the fortress) |
| **5** | **Pawnshop** | Middle-Right (2,1) | **Thief** — stealth & fencing |
| **6** | **Crucible** | Bottom-Left (0,2) | **Alchemist** — transformation (the vessel) |
| **7** | **Casino** | Bottom-Center (1,2) | **Merchant** — economic warfare |
| **8** | **Gala** | Bottom-Right (2,2) | **Bard** — rhythm & performance (the festival) |

#### Arena Navigation
```
Grid Layout (3×3):
[0] Labyrinth  [1] Guild House  [2] Sanctum
[3] Mountain   [4] Bastion      [5] Pawnshop  
[6] Crucible   [7] Casino       [8] Gala
```

#### Technical Implementation
- **ArenaName Enum**: Each arena uses a strongly-typed `ArenaName` enum instead of raw numeric indices
- **Type Safety**: Prevents invalid arena references and provides human-readable names
- **Index Conversion**: `ArenaName::as_u8()` provides the numeric index (0-8) when needed for calculations
- **Error Handling**: Invalid arena indices are handled gracefully with proper error messages

### Arena Navigation & Camera System
- **Arena Selection**: Use [ and ] keys to cycle through arenas (0-8, wraps around)
- **Camera Zoom**: Press P to toggle between single arena view and all-arenas overview
- **Visual Indicators**: Current arena highlighted with a white border when zoomed out
- **Smart Focus**: Camera automatically positions on current arena when zooming in
- **Character Memory**: Each arena remembers its last selected hero for seamless transitions

### Character Management Systems
- **Selection Toggle**: Tab cycles the Selected marker through heroes in the current arena (requires 2+ heroes)
- **Cross-Arena Movement**: Arrow-key movement seamlessly transitions heroes between adjacent arenas (edge-walk; opens the interrupt modal while recording)
- **Arena Boundaries**: Movement past edges teleports the character to the opposite side of the adjacent arena; the outer border of the 3×3 grid is clamped (no wraparound)
- **Re-parenting System**: Characters automatically become children of their current arena entity
- **State Preservation**: Heroes keep their selection when edge-walking between arenas

### Arena Update Logic
- **Event-Driven Updates**: Arena state refreshes on camera changes or arena transitions
- **Zoom-Out Behavior**: The overview changes only the camera and HUD theme — selection (and its ring) is untouched
- **Zoom-In Behavior**: Focuses the current arena; the selected hero keeps its ring
- **Empty Arena Handling**: Gracefully handles arenas with no characters present
- **Selection Visuals**: A glowing ring marks the selected hero; ghosts carry a blue ring

### Boss Mechanics
- **2-Minute Cycles**: Bosses operate on the same timing as your recordings
- **Deterministic Patterns**: Each boss has predictable, repeatable attack sequences
- **Telegraphed Attacks**: Visual warnings appear on grid tiles before major attacks
- **Pattern Recognition**: Success requires learning and countering boss rotations
- **No Enrage Timer**: Bosses reset each cycle without becoming stronger over time

### Arena-Specific Boss Types
0. **Labyrinth** (Hunter): precision ranged attacks and deadly trap mechanics — the Hollow Obelisk
1. **Guild House** (Guildmaster): the guild's **home** — a safe hearth, not a true boss; an abstract house/pyramid object
2. **Sanctum** (Cardinal): healing denial, purification attacks, and divine shields — the Torus Halo
3. **Mountain** (Forager): dynamically reshapes the battlefield through terrain manipulation and foraging — the Stepped Pyramid / Ziggurat
4. **Bastion** (Warrior): heavily armored defenses, charge attacks, and area damage — the Hexagonal Prism (the fortress)
5. **Pawnshop** (Thief): stealth mechanics, teleportation strikes, and ambush tactics — the Triangular Prism (wedge)
6. **Crucible** (Alchemist): elemental transformation and area denial through toxic pools — the Truncated Cone (the vessel)
7. **Casino** (Merchant): risk-reward mechanics and economic warfare strategies — the Hollow Icosphere (geode)
8. **Gala** (Bard): timing-sensitive responses to rhythmic attack patterns — the Capsule Resonator (the festival)

---

## Guild Management & Progression

### Gacha Recruitment System
- **Arena-Specific Recruitment**: Each arena only recruits heroes matching its class type
- **Battle Triggers**: Active combat in an arena generates gacha opportunities
- **Quality Tiers**: Heroes come in different rarity levels with enhanced abilities
- **Guild House Access**: Open recruitment boxes by pressing Enter and visiting guild house
- **Strategic Collection**: Build balanced rosters across all 8 character classes

### Character Development
- **Experience Growth**: Heroes gain levels through active participation in battles
- **Death Consequences**: Character death results in de-leveling, not permanent loss
- **Ability Evolution**: Higher-tier characters possess enhanced versions of base abilities
- **Equipment Systems**: Gear improvements provide statistical bonuses to character performance

### Guild House Operations
- **Management Hub**: Central location for all administrative functions
- **Recruitment Review**: Open and evaluate new character acquisitions
- **Global Buff Activation**: Use acquired consumables that affect all arenas simultaneously
- **Strategic Planning**: Review arena status and plan multi-arena coordination
- **Travel Planning**: Decide which arenas need heroes — travel itself is edge-walking

---

## Advanced Strategies

### Multi-Arena Coordination
- **Temporal Management**: Balance recording time across multiple arenas efficiently
- **Resource Allocation**: Distribute your best characters across priority arenas
- **Progressive Difficulty**: Master easier arenas before advancing to heroic/mythic tiers
- **Cross-Arena Learning**: Apply successful strategies from one arena to others

### Recording Optimization
- **Ability Timing**: Synchronize powerful abilities to create devastating combinations
- **Positioning Mastery**: Plan movement routes that maximize safety and effectiveness
- **Death Recovery**: Build recordings that account for potential character deaths
- **Cycle Efficiency**: Design recordings that smoothly transition between 2-minute cycles

### Team Composition Strategy
- **Tank-Healer-DPS**: Maintain classic MMO role balance in each arena
- **Class Synergy**: Combine complementary abilities for enhanced effectiveness
- **Backup Systems**: Include redundant healing and protection in case of deaths
- **Specialized Builds**: Develop arena-specific team compositions for unique challenges

---

## Combat Mechanics Deep Dive

### Grid-Based Tactical Combat
- **Tile Movement**: All positioning occurs on discrete grid squares
- **Line of Sight**: Abilities and attacks can be blocked by terrain features
- **Area of Effect**: Many abilities affect multiple adjacent tiles
- **Collision Rules**: Multiple characters can occupy the same grid cell
- **Environmental Hazards**: Some tiles contain traps, buffs, or damage zones

### Ability System Details
- **Cooldown Management**: Each ability has individual cooldown periods
- **Resource Costs**: Some abilities consume mana, stamina, or special resources
- **Cast Times**: Abilities have animation periods during which characters are vulnerable
- **Target Requirements**: Abilities may require specific targets (enemies, allies, empty tiles)
- **Upgrade Paths**: Higher-tier characters possess enhanced ability versions

### Death and Revival Mechanics
- **Death Consequences**: Characters lose levels and must restart from guild house
- **Revival Abilities**: Cardinals and other healers can resurrect fallen allies
- **Grid-Based Revival**: Revival spells target specific tiles rather than characters
- **Timing Requirements**: Revival must occur when dead character is present at target location
- **Recording Integration**: (Future) deaths and revivals will replay deterministically each cycle — derived from the intent timeline against the boss's fixed pattern, not stored as timeline events

---

## Progression & Victory Conditions

### Arena Mastery Progression
1. **Normal Tier**: Basic boss mechanics and standard difficulty
2. **Heroic Tier**: Enhanced boss abilities and additional mechanics
3. **Mythic Tier**: Maximum challenge with complex multi-phase encounters
4. **Perfect Runs**: Complete mastery demonstrated through flawless execution

### Long-Term Goals
- **Full Guild Development**: Recruit and develop a full roster of 320 heroes across all classes
- **Multi-Arena Excellence**: Successfully manage all 8 arenas simultaneously
- **Strategic Mastery**: Create sophisticated recording combinations across multiple cycles
- **Narrative Discovery**: Uncover the deeper story behind the arena conflicts

### Success Metrics
- **Boss Defeat Frequency**: Consistent victories across multiple 2-minute cycles
- **Character Survival**: Minimize deaths and level loss through strategic planning
- **Efficiency Optimization**: Achieve maximum damage/healing with minimal resource expenditure
- **Creative Problem Solving**: Develop innovative solutions to complex encounter mechanics

---

## Tips for New Players

### Essential Early Game Strategy
1. **Start Simple**: Master basic movement and single-character combat before recording
2. **Learn One Arena**: Focus on understanding one boss thoroughly before expanding
3. **Record Conservatively**: Create safe, reliable recordings rather than ambitious ones
4. **Study Patterns**: Observe boss timing carefully before committing to recordings
5. **Plan Positioning**: Always end recordings in safe locations for the next cycle

### Common Mistakes to Avoid
- **Rushing Recordings**: Take time to understand arena dynamics before committing
- **Ignoring Death Positions**: Don't leave ghosts in positions where they'll die repeatedly
- **Overcomplicating Early**: Simple, effective recordings outperform complex failures
- **Neglecting Other Arenas**: Passive arena management is crucial for overall progress
- **Poor Resource Management**: Balance advancement speed with character safety

### Mastery Development Path
1. **Movement Mastery**: Perfect grid-based positioning and timing
2. **Single-Character Combat**: Excel with individual heroes before team coordination
3. **Basic Recording**: Create simple, effective 2-minute action sequences
4. **Multi-Character Coordination**: Layer multiple recordings for enhanced effectiveness
5. **Advanced Strategy**: Develop sophisticated cross-arena management techniques

---

## Design Philosophy & Player Experience

### Intended Experience
Arenic transforms the complexity of managing massive raids into a solo experience that rewards strategic thinking, pattern recognition, and creative problem-solving. The game emphasizes:

- **Tactical Depth**: Every decision has cascading consequences across multiple timelines
- **Creative Expression**: Players develop unique solutions through recording combinations
- **Progressive Mastery**: Continuous improvement through iteration and refinement
- **Strategic Patience**: Success rewards careful planning over reactive gameplay

### Emotional Journey
- **Discovery Phase**: Wonder and experimentation with the recording system
- **Mastery Phase**: Growing confidence as patterns become familiar
- **Coordination Phase**: Satisfaction from successful multi-arena management
- **Innovation Phase**: Creative fulfillment from developing advanced strategies

The game rewards both analytical optimization and creative experimentation, providing a deeply satisfying experience for players who enjoy complex strategic challenges presented through elegant, accessible mechanics.

---

## Technical Architecture Principles

### Selected Character Query Pattern

**Architectural Rule**: All recording system operations query for the selected character (the `Selected` marker component) dynamically rather than storing or passing character entities as parameters.

#### Implementation Principle
```rust
// ✅ CORRECT - Query the Selected character when needed
pub enum RecordingRequest {
    Start,        // Query the Selected character when processing
    Stop { reason: StopReason },
    ShowModal,    // Query the Selected character when showing the modal
    Commit,       // Query the Selected character when committing
    Clear,        // Query the Selected character when clearing
}

// ❌ INCORRECT - Storing/passing entities
pub enum RecordingRequest {
    Start { entity: Entity },           // Don't store entities
    ShowModal { character: Entity },    // Don't pass character data
}
```

#### Rationale
1. **Single Source of Truth**: The `Selected` marker determines which character receives input
2. **R Key Behavior**: "R always operates on whoever is CURRENTLY selected when pressed"
3. **Dynamic Modal Behavior**: The modal always reflects the currently selected character
4. **Type Safety**: The `Single` system param guarantees exactly one selected character
5. **Simplicity**: Eliminates entity synchronization and parameter passing complexity

#### Query Pattern
```rust
// Standard query for selected-character operations (Bevy 0.18 Single param)
selected_q: Single<(Entity, Option<&Ghost>), With<Selected>>

// Usage in recording systems
let (character_entity, ghost_marker) = *selected_q;
```

#### Benefits
- **Architectural Consistency**: All recording operations follow the same pattern
- **Reduced Coupling**: No entity parameters between systems
- **Automatic Updates**: Modals and UI automatically reflect the currently selected character
- **Performance**: O(1) queries with minimal overhead
- **Maintainability**: Eliminates entity storage synchronization bugs

#### Application Scope
This pattern applies to:
- All `RecordingRequest` variants
- Modal state management (modal open ⇒ only the selected arena's clock pauses)
- Recording state tracking
- UI systems that need character context
- Any system that operates on "the current character"

---

## Conclusion

Arenic offers a unique gaming experience that combines the strategic depth of MMO raiding with innovative single-player mechanics. Through mastering the Record & Replay system, understanding character class synergies, and developing sophisticated multi-arena strategies, players can achieve the satisfaction of commanding massive coordinated raids while maintaining complete control over every aspect of their guild's performance.

Success in Arenic comes not from quick reflexes, but from careful planning, pattern recognition, and the ability to think several moves ahead across multiple simultaneous battlefields. Every recording matters, every position counts, and every strategic decision ripples across your entire guild's effectiveness.

Welcome to Arenic—where strategic mastery meets creative expression in the ultimate raid simulation experience.