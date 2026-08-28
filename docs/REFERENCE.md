# Reference research

The GDD supplied with this project (§2, §28) contains a teardown of *Câu Cá Vạn Cân* based on its
public store listings and published guides. This document records how that research was used.

## What was taken

Only **publicly described gameplay patterns**, which are ideas rather than protectable
expression:

- Portrait orientation and one-handed play.
- Tackle split into rod / reel / line / hook / float / bait, each upgradeable.
- A random pool of species per spot, with species having distinct behaviour.
- A fight governed by pulling force, line tension, line length and the risk of a snap.
- Fish weight tiers, from sub-kilogram up to very large.
- A team of anglers with roles and signature skills, over an RPG progression.

## What was not taken

Per GDD §2.2, and checked against the shipped content:

- No character names, artwork, logos, maps, UI composition, or copy.
- No skill names or effect wording.
- No balance tables, drop rates, or economy numbers. Every number in `assets/data` was derived
  from this project's own tier curves and validated by its own harness — see
  [BALANCE.md](BALANCE.md).
- No assets from the supplied image pack — see [ASSETS.md](ASSETS.md).

Species are real-world fish under their ordinary Vietnamese common names (Cá Rô Đồng, Cá Chép,
Cá Lăng, Cá Ngừ Đại Dương…), which nobody owns. The mythic and boss entries (Giao Long Con, Ẩn Hà
Ngư, Long Ngư Vương, Cá Tinh Hải) were written for this project.

## The design inference that mattered

The GDD's own reading — that the interesting part is not "big fish" but the **dynamic relation
between fish and tackle** — is what the simulation is built around, and it is why Auto is a policy
controller reading that relation rather than a reward tap. See
[ARCHITECTURE.md](ARCHITECTURE.md).

## Naming

See the trademark note at the end of [ASSETS.md](ASSETS.md): the working title is close to the
reference game's, which is a decision worth making explicitly before store submission.

## Sources

The store listings and guides enumerated in GDD §28. Nothing beyond publicly available
descriptions was used; the reference game was not decompiled, and no data was extracted from it.
