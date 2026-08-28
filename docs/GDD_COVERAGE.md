# GDD coverage

What the shipped code implements, section by section, and what it does not. The GDD describes a
full live-service game; this repository implements the playable core and the systems everything
else hangs off.

| GDD § | Topic | Status | Notes |
|---|---|---|---|
| 4 | Core loop | ✅ | Cast → bite → hook → fight → land/fail → loot → upgrade |
| 4.1 | Session states | ✅ | `SessionPhase`; all five failure modes reachable and tested |
| 5 | Fishing simulation | ✅ | `FishingSession`, deterministic and fixed-step |
| 5.1 | Fish state model | ✅ | `FishState` |
| 5.2 | Gear model | ✅ | `BuildStats`, six slots |
| 5.3 | Core formulas | ✅ | Implemented with tunables in `sim_config.json`; wear made quadratic, see BALANCE.md |
| 6 | Auto Fishing AI | ✅ | `AutoPilot`, five strategies, policy over the simulation |
| 6.1 | AI rules | ✅ | All five rules, plus phase-specific handling incl. the Trickster feint |
| 7 | Fish AI and behaviour | ✅ | Six archetypes, six phases, boss rotation is scripted |
| 8 | Character and team progression | ⚠️ | Level, talents, team formation, captain bonus, star scaling — but no awakening or recruitment/gacha |
| 8.1 | Team roles | ✅ | Five roles, each mapped to one simulation lever |
| 9 | Gear system | ⚠️ | Six slots, tiers, upgrade and purchase — no random affixes yet |
| 10 | World and spots | ✅ | Six zones, weighted pools, level gating, boss species per zone |
| 11 | Collection and economy | ✅ | Codex, seven rarity tiers, four currencies, audited ledger |
| 12 | Idle and offline | ✅ | Expected-value model, capped, idempotent, tested against live play |
| 13 | Social, guild, multiplayer | ❌ | Not implemented |
| 14 | Competitive modes | ⚠️ | `efficiencyScore()` exists for Perfect Auto; no leaderboards or modes |
| 15 | Events and live-ops | ❌ | Not implemented |
| 16 | Vertical UX and screen map | ✅ | Five screens, portrait, bottom nav, 96px minimum touch targets |
| 17 | Monetisation | ❌ | Not implemented, deliberately |
| 18 | Technical architecture | ⚠️ | Client complete. Stack differs from the GDD's Unity/C# proposal — see README |
| 19 | Server authority and anti-cheat | ⚠️ | Seams built (deterministic seeded sessions, single build-resolution point, idempotency keys, ledger). No server. |
| 20 | Core data model | ✅ | Player, Angler, Equipment, FishSpecies, FishInstance, FishingSpot, session, ledger |
| 21 | Service/API sketch | ❌ | No server |
| 22 | Content pipeline | ✅ | JSON tables, load-time validator, dangling-ID and progression checks, tested |
| 23 | QA and balance framework | ⚠️ | Simulation and balance testing done; no stress, cheat-input or economy-inflation harness |
| 24 | KPI and telemetry | ⚠️ | The simulation exposes the signals (catch rate, line-break rate, tension waste, efficiency); nothing collects them |
| 25 | Production roadmap | — | This repository covers P1 (prototype) and most of P2 (MVP) |
| 26 | Included HTML prototype | ➡️ | Superseded: the shipped client is the real thing on all four targets |
| 27 | Risks | ✅ | "Auto feels boring" is addressed by making the presets a genuine trade-off, and tested for |
| 28 | Research sources | ✅ | Recorded in REFERENCE.md; only publicly described mechanics used |

## The largest gaps, in the order worth closing them

1. **Server + authoritative settlement.** Everything else that is social or monetised depends on
   it, and the client is already shaped for it.
2. **Recruitment and gear affixes.** Progression currently ends once tier-6 gear is bought; there
   is no long-term sink.
3. **Guild and boss.** The boss species and multi-phase fights exist; the social layer does not.
4. **Live-ops and events.** Needs remote config and a schedule service.
5. **Audio and final art.** No audio at all today.
