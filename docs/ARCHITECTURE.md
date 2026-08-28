# Architecture

## The one rule

`core/sim` and `core/auto` **do not reference libGDX**. They are plain Java.

That is what makes the same simulation runnable in four places without a line of change:

1. the client, driving the visuals;
2. the JUnit suite, asserting determinism and balance;
3. the balance harness, sweeping millions of casts;
4. a JVM game server, re-simulating a session to verify what a client claimed.

Point 4 is the whole anti-cheat design (GDD §19). It only works if the simulation is
**deterministic**, so three properties are load-bearing:

- **Own PRNG.** `Rng` is SplitMix64, written out in the repo. `java.util.Random` is not
  bit-identical between the JVM and GWT's emulation, so a seed would replay differently on
  the server than on the client — which would make server verification produce false
  positives.
- **Fixed step.** `FishingSession.update` always advances by `SimConfig.tickSeconds`.
  `FishingController` accumulates real frame time and consumes it in whole ticks, so a 30 Hz
  phone and a 144 Hz desktop fight the same fish.
- **No wall-clock or floating frame deltas inside the sim.** Time enters only as ticks.

## Module map

```
core/
  sim/       FishingSession, FishState, Archetype, SimConfig, Rng, EncounterTable
  auto/      AutoPilot, AutoStrategy
  content/   ContentLoader, ContentValidator, GameContent, SpotDef, GearTemplate, AnglerDef
  meta/      PlayerState, LoadoutResolver, FishingController, CatchResolver,
             OfflineSettlement, SaveGame
  ui/        Theme, Art, Ui
  screen/    BaseScreen, ScrollScreen, FishingScreen, MapScreen, GearScreen,
             TeamScreen, CodexScreen
  platform/  SaveStore
```

Dependency direction is strictly downward: `screen` → `ui` → `meta` → `content` → `sim`.
Nothing in `sim` knows about anything above it.

## The meta/sim seam

`LoadoutResolver.resolve(player, content)` is the **only** place an inventory becomes
simulation input. It flattens gear, team, and talents into one `BuildStats`.

This matters for more than tidiness. When the server-authoritative build lands, the server
recomputes `BuildStats` from its own copy of the player's inventory and compares it against
what the client used. A client that inflates its rod power has to lie in exactly one place,
and that place is checked.

## GWT constraints

Everything under `com.vancan.autofishing` is transpiled to JavaScript, so it must stay inside
GWT's JRE emulation:

| Not available | What the code does instead |
|---|---|
| Reflection | `ContentLoader` reads every field explicitly; `SaveGame` writes JSON by hand |
| `java.util.stream` | Plain loops |
| `String.format` | `Ui.trim1` / `Ui.number` format by hand |
| Native libraries | No FreeType — the font is a pre-baked bitmap atlas |
| `System.nanoTime` | `VanCanGame.nowMillis()` is the single time source |

`core/src/main/java/com/vancan/autofishing/Core.gwt.xml` declares the module with
`<source path="" />`, which compiles the package and everything beneath it. A class added
under this package that breaks a rule above fails the GWT compile, not the JVM one — so run
`./gradlew :html:compileGwt` before assuming a change is portable.

## Persistence

`SaveStore` writes the `SaveGame` JSON blob through libGDX `Preferences`. `Preferences` is the
only storage API with a working implementation on all four backends — on HTML5 it maps to local
storage, where a plain file write does not exist.

The payload is ordinary JSON, so moving to a server-side save later means changing `SaveStore`
and nothing above it. A corrupt or future-version save logs and starts fresh rather than
crashing on every launch with no way back.

## Rendering

One `ExtendViewport(1080, 1920)`: the design width is fixed and the world grows vertically on
screens taller than 9:16, so no portrait phone gets letterboxed. Screen chrome anchors to
`viewport.getWorldHeight()`, **not** to the design constant — anchoring to the constant leaves
an undrawn band on tall screens.

The UI is immediate mode (`Ui`) rather than Scene2D. Scene2D would mean a skin pipeline and a
retained widget tree for what is mostly rectangles recomputed every frame from live simulation
state; keeping the HUD code beside the state it reads is worth more here than the widget
library is.

Art is generated procedurally at start-up (`Art`) — see [ASSETS.md](ASSETS.md) for why.

## What is deliberately not built yet

Guild, world boss, tournaments, live-ops, the server, and monetisation. The seams they need
already exist — deterministic sessions with seeds, an idempotent economy ledger, a single
build-resolution point, config-driven balance — but none of the features are implemented.
See [GDD_COVERAGE.md](GDD_COVERAGE.md) for the full mapping.
