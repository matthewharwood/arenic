# Boss Schematic Prompt Remap

This remaps the revised Hollow Obelisk prompt structure across the full 3x3 arena
roster. Each entry keeps the same top-down schematic grammar:

- `HollowStructure`
- `Faceting`
- `SurfaceDetail`
- `Behavior`
- `Mood`, `Floor`, `SignatureMark`, `Atmosphere`

Use these as prompt fragments for orthographic plan-view boss sheets: high-key
white floor, dark readable shell, visible internal footprint, crisp diagnostic
linework, and projected combat telegraph overlays.

Canonical arena order:

| Index | Arena | Class | Boss | Behavior |
|---:|---|---|---|---|
| 0 | Labyrinth | Hunter | Hollow Obelisk | ObeliskBladeProjection |
| 1 | Guild House | Guildmaster | Home Pyramid | HomeHearthProjection |
| 2 | Sanctum | Cardinal | Torus Halo | HaloFillProjection |
| 3 | Mountain | Forager | Stepped Pyramid | ZigguratGrowProjection |
| 4 | Bastion | Warrior | Hexagonal Prism | PrismBlockProjection |
| 5 | Pawnshop | Thief | Triangular Prism | WedgeBeamProjection |
| 6 | Crucible | Alchemist | Truncated Cone | CauldronRiseProjection |
| 7 | Casino | Merchant | Hollow Icosphere | GeodeShimmerProjection |
| 8 | Gala | Bard | Capsule Resonator | CapsulePulseProjection |

## Shared Schematic Baseline

```xml
<SchematicBaseline>
  <View value="top-down orthographic plan view"/>
  <Composition value="centered boss footprint with projected telegraph paths extending across a high-key floor"/>
  <Lighting value="sterile technical studio lighting, high visibility"/>
  <Linework value="crisp perimeter lines, interior cut-away lines, measurement gridlines, trap or mechanic indicators"/>
  <MaterialRule value="dark matte shell for contrast, luminous class-color projection overlays"/>
  <NegativePrompt value="no character anatomy, no faces, no limbs, no fantasy painting haze, no decorative clutter, no perspective camera"/>
</SchematicBaseline>
```

## 0. Labyrinth / Hunter / Hollow Obelisk

```xml
<HollowStructure>
  <Hollow value="true"/>
  <InteriorVisibility lever="aperture_openness" range="0.2-1.0" value="high (internal cavity visible as plan-cut)"/>
  <WallThickness lever="wall_thickness" range="0.05-0.35" value="thin-to-medium (as defined by obelisk structure)"/>
  <Cutouts lever="cutout_count" range="0-8" value="plan-view cross-section of vertical slit windows"/>
  <VoidShape value="central vertical void (cut-away plan view)"/>
</HollowStructure>

<Faceting>
  <FacetCount value="4 primary faces (visible as perimeter lines)"/>
  <EdgeSharpness lever="edge_bevel" range="0.0-0.25" value="crisp, technical lines"/>
  <SurfacePlanes value="clean, readable, monolithic material"/>
  <Asymmetry lever="asymmetry" range="0.0-0.4" value="low, schematic precision"/>
</Faceting>

<SurfaceDetail>
  <MaterialFamily value="matte dark stone / blackened metal"/>
  <SurfaceFinish lever="roughness" range="0.35-0.95" value="matte dark shell (to provide contrast)"/>
  <Wear lever="erosion_level" range="0.0-0.8" value="minimal, technical diagram precision"/>
  <Engraving lever="engraving_density" range="0.0-1.0" value="precise schematic gridlines and trap indicators"/>
  <Damage lever="chip_amount" range="0.0-0.5" value="none, diagnostic schematic quality"/>
</SurfaceDetail>
```

```xml
<Behavior>
  <Name>ObeliskBladeProjection</Name>
  <Motion>blade of light rotating within the schematic footprint cross-section</Motion>
  <TelegraphRead>crisp rotating sightline projection corridor and trap zone overlay</TelegraphRead>
  <Speed lever="telegraph_speed" range="0.2-2.0" value="slow deliberate schematic rotation"/>
  <Coverage lever="light_coverage" range="0.1-1.0" value="one face projection at a time (on high-key floor)"/>
</Behavior>
```

```xml
<Mood>precise diagrammatic clarity, controlled field visualization, isolated diagnostic precision</Mood>
<Floor>pristine white composite, perfect high-key surface for isolation</Floor>
<SignatureMark>precise cyan projected sightline path and trap zone indicators</SignatureMark>
<Atmosphere>clean air, sterile lighting, technical grid lines, high visibility</Atmosphere>
```

## 1. Guild House / Guildmaster / Home Pyramid

Guild House is the safe home object, not a hostile boss. Keep the same schematic
format so the full arena set stays comparable.

```xml
<HollowStructure>
  <Hollow value="false"/>
  <InteriorVisibility lever="aperture_openness" range="0.0-0.2" value="none (solid pyramid footprint, no internal cavity)"/>
  <WallThickness lever="wall_thickness" range="0.25-1.0" value="solid mass (home object, not hollow shell)"/>
  <Cutouts lever="cutout_count" range="0-0" value="none"/>
  <VoidShape value="none (central hearth ring replaces void)"/>
</HollowStructure>

<Faceting>
  <FacetCount value="4 roof planes (square pyramid plan diagram)"/>
  <EdgeSharpness lever="edge_bevel" range="0.05-0.35" value="soft technical lines, calm home geometry"/>
  <SurfacePlanes value="smooth, stable, centered pyramid mass"/>
  <Asymmetry lever="asymmetry" range="0.0-0.1" value="none to very low, safe and balanced"/>
</Faceting>

<SurfaceDetail>
  <MaterialFamily value="warm dark wood / clay / hearth stone"/>
  <SurfaceFinish lever="roughness" range="0.45-0.95" value="matte warm home surface"/>
  <Wear lever="erosion_level" range="0.0-0.4" value="gentle lived-in texture, no combat damage"/>
  <Engraving lever="engraving_density" range="0.0-0.5" value="hearth-ring inlay and home registration marks"/>
  <Damage lever="chip_amount" range="0.0-0.1" value="none"/>
</SurfaceDetail>
```

```xml
<Behavior>
  <Name>HomeHearthProjection</Name>
  <Motion>static or gently breathing hearth-ring glow inside the schematic footprint</Motion>
  <TelegraphRead>safe-zone anchor, no hostile lane, no hazard overlay</TelegraphRead>
  <Speed lever="telegraph_speed" range="0.0-0.4" value="still, calm, nearly motionless"/>
  <Coverage lever="light_coverage" range="0.1-0.4" value="central hearth ring only"/>
</Behavior>
```

```xml
<Mood>warm diagnostic calm, safe home marker, centered non-hostile clarity</Mood>
<Floor>pristine white composite, perfect high-key surface with a faint hearth registration ring</Floor>
<SignatureMark>soft amber hearth-ring inlay and safe-zone indicator</SignatureMark>
<Atmosphere>clean air, sterile lighting, no threat overlay, high visibility</Atmosphere>
```

## 2. Sanctum / Cardinal / Torus Halo

```xml
<HollowStructure>
  <Hollow value="true"/>
  <InteriorVisibility lever="aperture_openness" range="0.45-1.0" value="very high (large central aperture dominates the plan-cut)"/>
  <WallThickness lever="wall_thickness" range="0.05-0.25" value="thin even annular wall"/>
  <Cutouts lever="cutout_count" range="0-8" value="radial reliquary breaks and liturgical tick apertures"/>
  <VoidShape value="large circular central void (perfectly readable in top-down plan)"/>
</HollowStructure>

<Faceting>
  <FacetCount value="continuous halo perimeter with 12-24 radial schematic segments"/>
  <EdgeSharpness lever="edge_bevel" range="0.0-0.18" value="precise concentric ring lines"/>
  <SurfacePlanes value="thin, even, polished annulus"/>
  <Asymmetry lever="asymmetry" range="0.0-0.12" value="very low, sacred radial symmetry"/>
</Faceting>

<SurfaceDetail>
  <MaterialFamily value="matte black basalt / blackened metal / restrained holy gold inlay"/>
  <SurfaceFinish lever="roughness" range="0.35-0.85" value="dark satin shell with clean gold projection contrast"/>
  <Wear lever="erosion_level" range="0.0-0.4" value="minimal ceremonial wear"/>
  <Engraving lever="engraving_density" range="0.2-0.9" value="concentric liturgical ticks, purification rings, shield boundary marks"/>
  <Damage lever="chip_amount" range="0.0-0.15" value="none to barely visible"/>
</SurfaceDetail>
```

```xml
<Behavior>
  <Name>HaloFillProjection</Name>
  <Motion>ring fills, resolves, then resets within the annular schematic footprint</Motion>
  <TelegraphRead>purification radius, healing denial ring, divine shield closure</TelegraphRead>
  <Speed lever="telegraph_speed" range="0.2-1.2" value="ceremonial radial fill"/>
  <Coverage lever="light_coverage" range="0.25-1.0" value="annular sectors or full concentric ring fill"/>
</Behavior>
```

```xml
<Mood>reverent clinical diagram, sacred radial order, controlled containment</Mood>
<Floor>pristine white composite, perfect high-key surface with faint gold measurement rings</Floor>
<SignatureMark>concentric gold-inlay ring beneath the halo</SignatureMark>
<Atmosphere>clean air, sterile lighting, fine gilt grid dust translated into schematic dots</Atmosphere>
```

## 3. Mountain / Forager / Stepped Pyramid

```xml
<HollowStructure>
  <Hollow value="true"/>
  <InteriorVisibility lever="aperture_openness" range="0.2-0.75" value="medium (central shaft visible through nested tiers)"/>
  <WallThickness lever="wall_thickness" range="0.12-0.35" value="medium-to-thick tiered stone walls"/>
  <Cutouts lever="cutout_count" range="0-8" value="terrace seams, step breaks, square shaft vents"/>
  <VoidShape value="central square shaft surrounded by nested-square terrace rings"/>
</HollowStructure>

<Faceting>
  <FacetCount value="3-7 concentric square tiers (visible as contour lines)"/>
  <EdgeSharpness lever="edge_bevel" range="0.02-0.22" value="crisp but mineral, readable terrace edges"/>
  <SurfacePlanes value="stacked square planes, ziggurat contour map"/>
  <Asymmetry lever="asymmetry" range="0.0-0.3" value="low-to-medium, geological but still schematic"/>
</Faceting>

<SurfaceDetail>
  <MaterialFamily value="earthen dark basalt / mineral stone / dark geologic composite"/>
  <SurfaceFinish lever="roughness" range="0.55-0.95" value="matte stone with sparse green projection accents"/>
  <Wear lever="erosion_level" range="0.1-0.7" value="controlled erosion along tier edges"/>
  <Engraving lever="engraving_density" range="0.2-0.9" value="terrain contour lines, resource nodes, fault-routing marks"/>
  <Damage lever="chip_amount" range="0.0-0.4" value="small mineral chips, not ruin damage"/>
</SurfaceDetail>
```

```xml
<Behavior>
  <Name>ZigguratGrowProjection</Name>
  <Motion>light grows from the central shaft through nested-square tiers</Motion>
  <TelegraphRead>terrain rise pattern, harvest route reveal, safe and unsafe contour overlay</TelegraphRead>
  <Speed lever="telegraph_speed" range="0.2-1.4" value="slow geological expansion"/>
  <Coverage lever="light_coverage" range="0.2-1.0" value="center-out tier growth across square terraces"/>
</Behavior>
```

```xml
<Mood>terrain survey diagram, geological clarity, controlled growth visualization</Mood>
<Floor>pristine white composite, perfect high-key surface with subtle topographic guide grid</Floor>
<SignatureMark>off-axis green geological fault-vein and contour hazard indicators</SignatureMark>
<Atmosphere>clean air, sterile lighting, survey lines, high visibility</Atmosphere>
```

## 4. Bastion / Warrior / Hexagonal Prism

```xml
<HollowStructure>
  <Hollow value="true"/>
  <InteriorVisibility lever="aperture_openness" range="0.15-0.65" value="medium-low (heavy armored shell with visible core chamber)"/>
  <WallThickness lever="wall_thickness" range="0.18-0.35" value="thick hexagonal fortress wall"/>
  <Cutouts lever="cutout_count" range="0-6" value="six face ports, shield slots, directional block indicators"/>
  <VoidShape value="compact central chamber inside a heavy hex footprint"/>
</HollowStructure>

<Faceting>
  <FacetCount value="6 primary faces (each readable as a blockable direction)"/>
  <EdgeSharpness lever="edge_bevel" range="0.02-0.2" value="hard armor edges with controlled bevels"/>
  <SurfacePlanes value="heavy hexagonal armor plates, fortress-like mass"/>
  <Asymmetry lever="asymmetry" range="0.0-0.18" value="low, disciplined defensive geometry"/>
</Faceting>

<SurfaceDetail>
  <MaterialFamily value="quenched basalt / blackened armor / forge-dark metal"/>
  <SurfaceFinish lever="roughness" range="0.4-0.9" value="matte armored shell with molten orange projections"/>
  <Wear lever="erosion_level" range="0.0-0.5" value="battle-tested edges without losing clean diagram read"/>
  <Engraving lever="engraving_density" range="0.1-0.8" value="hex shield marks, block arrows, charge lane registration"/>
  <Damage lever="chip_amount" range="0.0-0.35" value="small armor nicks only"/>
</SurfaceDetail>
```

```xml
<Behavior>
  <Name>PrismBlockProjection</Name>
  <Motion>one hex face lights to show the blocked direction inside the schematic footprint</Motion>
  <TelegraphRead>directional shield wall, blocked face, charge warning wedge</TelegraphRead>
  <Speed lever="telegraph_speed" range="0.2-1.6" value="deliberate face-to-face defensive stepping"/>
  <Coverage lever="light_coverage" range="0.1-0.5" value="one hex face or one outward wedge at a time"/>
</Behavior>
```

```xml
<Mood>armory diagnostic plate, defensive clarity, furnace power under control</Mood>
<Floor>pristine white composite, perfect high-key surface with a hex measurement grid</Floor>
<SignatureMark>centered anvil-hexagon sigil with orange block and charge indicators</SignatureMark>
<Atmosphere>clean air, sterile lighting, no smoke, orange hazard overlays only</Atmosphere>
```

## 5. Pawnshop / Thief / Triangular Prism

```xml
<HollowStructure>
  <Hollow value="true"/>
  <InteriorVisibility lever="aperture_openness" range="0.2-0.8" value="medium-high (slit cavity visible along wedge axis)"/>
  <WallThickness lever="wall_thickness" range="0.06-0.25" value="thin-to-medium stealth shell"/>
  <Cutouts lever="cutout_count" range="0-8" value="narrow lockpick slots, wedge vents, hidden path cuts"/>
  <VoidShape value="triangular inner void aligned to the wedge point"/>
</HollowStructure>

<Faceting>
  <FacetCount value="3 primary faces and one dominant forward point"/>
  <EdgeSharpness lever="edge_bevel" range="0.0-0.18" value="knife-clean technical wedge lines"/>
  <SurfacePlanes value="triangular stealth plates, directional wedge footprint"/>
  <Asymmetry lever="asymmetry" range="0.0-0.35" value="controlled offset, sneaky without becoming noisy"/>
</Faceting>

<SurfaceDetail>
  <MaterialFamily value="worn black slate / blackened metal / dark fencing-den composite"/>
  <SurfaceFinish lever="roughness" range="0.45-0.95" value="matte dark shell with cyan and small warm projections"/>
  <Wear lever="erosion_level" range="0.0-0.55" value="subtle worn edges"/>
  <Engraving lever="engraving_density" range="0.15-0.9" value="lockpick paths, ambush lanes, hidden route ticks"/>
  <Damage lever="chip_amount" range="0.0-0.25" value="minor scuffs, no ruin damage"/>
</SurfaceDetail>
```

```xml
<Behavior>
  <Name>WedgeBeamProjection</Name>
  <Motion>narrow beam projects forward and back along the wedge axis</Motion>
  <TelegraphRead>ambush corridor, theft lane, stealth strike line</TelegraphRead>
  <Speed lever="telegraph_speed" range="0.4-2.0" value="furtive snap-and-hold beam"/>
  <Coverage lever="light_coverage" range="0.1-0.6" value="thin forward-back corridor, not a wide fan"/>
</Behavior>
```

```xml
<Mood>covert technical diagram, controlled suspicion, precise ambush geometry</Mood>
<Floor>pristine white composite, perfect high-key surface with discreet pawnshop registration marks</Floor>
<SignatureMark>cyan watch-band lane with a rationed warm torch-seam crack</SignatureMark>
<Atmosphere>clean air, sterile lighting, shadow translated into line density only</Atmosphere>
```

## 6. Crucible / Alchemist / Truncated Cone

```xml
<HollowStructure>
  <Hollow value="true"/>
  <InteriorVisibility lever="aperture_openness" range="0.25-0.85" value="high (small mouth and inner vessel visible in plan)"/>
  <WallThickness lever="wall_thickness" range="0.15-0.35" value="thick-walled conical vessel"/>
  <Cutouts lever="cutout_count" range="0-8" value="pour channels, pipette vents, alchemical drain slots"/>
  <VoidShape value="small circular mouth inside a heavy circular vessel footprint"/>
</HollowStructure>

<Faceting>
  <FacetCount value="16-32 radial schematic segments approximating a round truncated cone"/>
  <EdgeSharpness lever="edge_bevel" range="0.02-0.22" value="technical rim lines with slight vessel softness"/>
  <SurfacePlanes value="thick annular rim, inner pool, outer vessel wall"/>
  <Asymmetry lever="asymmetry" range="0.0-0.25" value="low, with optional controlled spill offset"/>
</Faceting>

<SurfaceDetail>
  <MaterialFamily value="black-green ceramic / oxidized metal / dark brine-stained stone"/>
  <SurfaceFinish lever="roughness" range="0.4-0.9" value="matte vessel shell with acid-lime projection"/>
  <Wear lever="erosion_level" range="0.0-0.6" value="chemical etching, controlled and readable"/>
  <Engraving lever="engraving_density" range="0.2-1.0" value="alchemy sigils, pool boundaries, reaction grid marks"/>
  <Damage lever="chip_amount" range="0.0-0.25" value="minor rim wear only"/>
</SurfaceDetail>
```

```xml
<Behavior>
  <Name>CauldronRiseProjection</Name>
  <Motion>liquid light rises and falls inside the vessel, shown as pulsing pool rings in plan</Motion>
  <TelegraphRead>toxic pool growth, transformation radius, acid overflow warning</TelegraphRead>
  <Speed lever="telegraph_speed" range="0.2-1.5" value="viscous rise-and-fall pulse"/>
  <Coverage lever="light_coverage" range="0.2-1.0" value="central pool rings expanding into spill zones"/>
</Behavior>
```

```xml
<Mood>sterile lab diagram, reaction map clarity, toxic area-denial visualization</Mood>
<Floor>pristine white composite, perfect high-key surface with alchemical measurement grid</Floor>
<SignatureMark>acid-lime alchemist sigil with pool boundary and reaction indicators</SignatureMark>
<Atmosphere>clean air, sterile lighting, no smog, lime reaction overlays only</Atmosphere>
```

## 7. Casino / Merchant / Hollow Icosphere

```xml
<HollowStructure>
  <Hollow value="true"/>
  <InteriorVisibility lever="aperture_openness" range="0.2-0.75" value="medium (faceted geode cavity visible through polygon apertures)"/>
  <WallThickness lever="wall_thickness" range="0.08-0.3" value="medium faceted shell"/>
  <Cutouts lever="cutout_count" range="0-8" value="facet windows, tally slots, coin-slit apertures"/>
  <VoidShape value="low polygonal geode chamber with fractured central opening"/>
</HollowStructure>

<Faceting>
  <FacetCount value="12-20 low geode facets visible as polygon cells"/>
  <EdgeSharpness lever="edge_bevel" range="0.0-0.2" value="crisp faceted gem lines"/>
  <SurfacePlanes value="low faceted geode planes, odds-table cells"/>
  <Asymmetry lever="asymmetry" range="0.05-0.4" value="medium controlled asymmetry for luck and volatility"/>
</Faceting>

<SurfaceDetail>
  <MaterialFamily value="dark plum geode / blackened polished stone / restrained gold metal"/>
  <SurfaceFinish lever="roughness" range="0.35-0.85" value="matte-to-satin dark shell with gold and cyan projections"/>
  <Wear lever="erosion_level" range="0.0-0.45" value="clean casino artifact, lightly faceted edges"/>
  <Engraving lever="engraving_density" range="0.2-1.0" value="tally marks, odds grids, payout cells, coin arcs"/>
  <Damage lever="chip_amount" range="0.0-0.3" value="minor geode chips only"/>
</SurfaceDetail>
```

```xml
<Behavior>
  <Name>GeodeShimmerProjection</Name>
  <Motion>facets flicker semi-randomly across the schematic footprint</Motion>
  <TelegraphRead>volatile payout cells, luck zones, risk-reward scatter map</TelegraphRead>
  <Speed lever="telegraph_speed" range="0.5-2.0" value="quick uneven shimmer with deterministic patterning"/>
  <Coverage lever="light_coverage" range="0.15-0.85" value="scattered active facets, never all at once by default"/>
</Behavior>
```

```xml
<Mood>odds-table diagnostic diagram, controlled volatility, cold accounting clarity</Mood>
<Floor>pristine white composite, perfect high-key surface with faint casino grid and tally registration</Floor>
<SignatureMark>asynchronous gold and cyan tally sparkle, explicitly no beat</SignatureMark>
<Atmosphere>clean air, sterile lighting, no smoke, projected odds cells only</Atmosphere>
```

## 8. Gala / Bard / Capsule Resonator

```xml
<HollowStructure>
  <Hollow value="true"/>
  <InteriorVisibility lever="aperture_openness" range="0.25-0.85" value="high (central filament visible inside capsule footprint)"/>
  <WallThickness lever="wall_thickness" range="0.05-0.25" value="thin-to-medium ribbed capsule shell"/>
  <Cutouts lever="cutout_count" range="0-8" value="rib windows, beat slots, resonator vents"/>
  <VoidShape value="long capsule void with a central filament line"/>
</HollowStructure>

<Faceting>
  <FacetCount value="rounded capsule perimeter with 4-16 rhythmic rib divisions"/>
  <EdgeSharpness lever="edge_bevel" range="0.02-0.25" value="clean capsule edge, softer than wedge or obelisk"/>
  <SurfacePlanes value="pill-shaped resonator shell, internal filament track"/>
  <Asymmetry lever="asymmetry" range="0.0-0.25" value="low, rhythmic balance with optional performance offset"/>
</Faceting>

<SurfaceDetail>
  <MaterialFamily value="dark violet shell / blackened stage metal / neon resonator material"/>
  <SurfaceFinish lever="roughness" range="0.35-0.85" value="matte dark capsule with pink and cyan emission"/>
  <Wear lever="erosion_level" range="0.0-0.35" value="clean performance object, no ruin wear"/>
  <Engraving lever="engraving_density" range="0.15-0.9" value="beat ticks, timing lanes, resonant seam marks"/>
  <Damage lever="chip_amount" range="0.0-0.15" value="none to minimal"/>
</SurfaceDetail>
```

```xml
<Behavior>
  <Name>CapsulePulseProjection</Name>
  <Motion>central filament pulses on the beat and activates capsule ribs</Motion>
  <TelegraphRead>beat window, rhythm lane, synchronized pulse hazard</TelegraphRead>
  <Speed lever="telegraph_speed" range="0.5-2.0" value="clear rhythmic pulse cadence"/>
  <Coverage lever="light_coverage" range="0.2-1.0" value="filament first, then ribs or capsule-wide pulse"/>
</Behavior>
```

```xml
<Mood>rhythm diagnostic chart, beat-locked clarity, isolated festival mechanics</Mood>
<Floor>pristine white composite, perfect high-key surface with timing-grid registration</Floor>
<SignatureMark>seams brightening on the beat in pink and cyan</SignatureMark>
<Atmosphere>clean air, sterile lighting, no haze, beat pulses as diagrammatic overlays</Atmosphere>
```

