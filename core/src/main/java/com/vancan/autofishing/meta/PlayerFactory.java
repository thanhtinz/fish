package com.vancan.autofishing.meta;

import com.vancan.autofishing.content.AnglerDef;
import com.vancan.autofishing.content.GameContent;
import com.vancan.autofishing.content.GearSlot;
import com.vancan.autofishing.content.GearTemplate;
import com.vancan.autofishing.content.SpotDef;
import com.vancan.autofishing.sim.SkillDef;

/** Builds the starting state for a brand-new player. */
public final class PlayerFactory {

    private PlayerFactory() {
    }

    public static PlayerState newPlayer(GameContent content, long now) {
        PlayerState p = new PlayerState();
        p.lastSeenAt = now;
        p.currencies.put(Currency.GOLD, 250);
        p.currencies.put(Currency.GEMS, 30);
        p.currencies.put(Currency.TICKETS, 3);
        p.currencies.put(Currency.ESSENCE, 0);

        for (GearSlot slot : GearSlot.values()) {
            GearTemplate template = content.starterGear(slot);
            if (template == null) continue;
            String ownedId = "own_" + template.id;
            p.gear.put(ownedId, new OwnedGear(ownedId, template.id, 1));
            p.loadout.equipped.put(slot, ownedId);
        }

        // Seed the two lowest-rarity anglers so the team screen is meaningful from the first run
        // and the Striker/Controller trade-off is visible immediately.
        int seeded = 0;
        for (AnglerDef def : content.anglers.values()) {
            if (def.rarity.ordinal() > 0 || seeded >= 2) continue;
            String ownedId = "own_" + def.id;
            p.anglers.put(ownedId, new OwnedAngler(ownedId, def.id, 1, 1));
            p.loadout.team.add(ownedId);
            seeded++;
        }

        for (SkillDef skill : content.skills.values()) {
            if (p.loadout.skills.size() >= Loadout.MAX_SKILLS) break;
            p.loadout.skills.add(skill.id);
        }

        for (SpotDef spot : content.spots.values()) {
            p.currentSpotId = spot.id;
            break;
        }
        return p;
    }

    /** A spot is available once the player is high enough level (GDD 10). */
    public static boolean isUnlocked(PlayerState player, SpotDef spot) {
        return player.level >= spot.unlockLevel;
    }
}
