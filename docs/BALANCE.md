# Balance

All coefficients live in `assets/data/sim_config.json` and are loaded into `SimConfig`. Nothing
is hard-coded in the client, so a rebalance is a config push (GDD §5.3, §22).

## The tension model

Tension is the number the whole game turns on.

```
fishForce      = power × phaseForceMultiplier × fatigue
tensionTarget  = fishForce + effectivePull × pullTensionCoefficient
                 − safetyMitigation − elasticity × elasticityRelief
tension       += (tensionTarget − tension) × (lineResponse / control) × dt
```

Plus an **instant shock** on every phase change, proportional to the jump in force.

Damage:

```
effectivePull = (rodPower + reelPower + teamPull) × pullLevel
damage        = effectivePull × damageCoefficient × damageBonus × crit
staminaDrain  = effectivePull × staminaDamageCoefficient
```

Line wear is **quadratic** in overshoot above the safe threshold:

```
overshoot = (tension − safeTension) / breakingTension
integrity -= overshoot² × wearRate × dt
```

A cast is lost when the line snaps (hard limit or worn through), the fish empties the spool,
the hook is missed, the fish escapes a slack line, or the fight times out.

## Three findings that shaped the model

These came out of running the harness, not out of the spec. Each was a case where the obvious
formulation produced a game with no decisions in it.

**1. Pull has to dominate tension, or the strategies collapse.**
With a low `pullTensionCoefficient`, backing off barely reduced snap risk while clearly reducing
damage, so the Safe policy was strictly worse than Aggressive and all five presets produced
near-identical catch rates. Raising it to 1.7 made easing off a real trade. *Pillar: Auto is not
idle-only.*

**2. Wear must be quadratic, not linear.**
Under linear wear the Safe policy snapped as many lines as the Aggressive one: its overshoots
were small, but its fights ran ~60% longer and the totals evened out. Squaring the overshoot
makes brushing the limit nearly free and riding it expensive, which is the gradient the presets
are meant to trade along. It also matches how material fatigue behaves.

**3. A running fish must be catchable.**
At the original reel speeds every fish outran every reel during a run phase, so the spool decided
fights regardless of play and Safe lost ~150 fish per zone to it. Reel speeds went up, line
lengths up, and the run phase's drive multiplier came down from 2.30 to 1.80.

A fourth, smaller one: the bite wait was a plain exponential — right mean, heavy tail, 11% of
casts spent over ten seconds staring at an empty pond. It is now Erlang-2 (two half-mean
exponentials): same mean, half the variance, 6.7% tail.

## Tier curves

Six gear tiers, one per zone. `tools/gen_species.py` derives every species stat block from these,
so a rebalance means editing one curve rather than reconciling 32 stat blocks by hand.

| Tier | Line strength | Rod+reel power | Reel speed | Line length |
|---|---|---|---|---|
| 1 | 60 | 16 | 3.8 | 34 |
| 2 | 85 | 25 | 4.5 | 39 |
| 3 | 120 | 37 | 5.4 | 44 |
| 4 | 165 | 52 | 6.3 | 49 |
| 5 | 225 | 70 | 7.4 | 54 |
| 6 | 300 | 92 | 8.6 | 59 |

Species power is derived as `0.297 × lineStrength` for the tier, which keeps a raging fish at
roughly 55% of the breaking point on matched gear — enough headroom that how hard you pull is
the player's decision rather than the tier's.

## Targets, and the tests that hold them

`BalanceTest` asserts wide guard-rails, not exact numbers: it exists to catch a change that makes
a zone unplayable or trivial, not to freeze the tuning.

| Property | Guard-rail |
|---|---|
| On-tier catch rate | 55–95%, fight 4–45s |
| Undergeared farming | gold/min below 75% of on-tier |
| Safe vs Aggressive | Safe is always slower; Safe out-lands Aggressive in some contested zone |
| Risk | On tier, Aggressive snaps at least as many lines as Safe |
| Gear progression | Upgrading a tier never lowers the catch rate |
| Failure modes | All of snap / spool / miss / land are reachable |

One test deliberately pins the *opposite* case: `undergearedLeavesNoSafeHeadroom`. One tier
behind, the fish's own force exceeds the wear threshold before the angler pulls at all, so no
pull level keeps the line intact and every policy converges. That is the signal telling a player
to go upgrade rather than change tactics, and it should not be "fixed" by accident.

## Working on balance

```bash
./gradlew :core:balanceReport   # full grid: catch rate, fight length, gold/min, failure mix
./gradlew :core:test            # guard-rails
```

`assets/data` is declared as an input to `:core:test`, so editing a table re-runs the suite. It
was not, at first, and Gradle happily cached a green run across content changes — which defeats
the entire point of having these tests.

## Current measured behaviour

On-tier, Balanced, per zone: **72–75% landed**, 16–22 s per fight. Aggressive lands ~4 points
fewer but finishes ~35% faster, so it wins on gold/minute while Balanced and Safe win on not
losing a specific fish — which is what makes the choice interesting when the fish is rare.
