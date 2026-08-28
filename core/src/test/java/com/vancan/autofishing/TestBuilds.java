package com.vancan.autofishing;

import com.vancan.autofishing.content.GameContent;
import com.vancan.autofishing.content.GearSlot;
import com.vancan.autofishing.content.GearTemplate;
import com.vancan.autofishing.meta.Loadout;
import com.vancan.autofishing.meta.LoadoutResolver;
import com.vancan.autofishing.meta.OwnedGear;
import com.vancan.autofishing.meta.PlayerState;
import com.vancan.autofishing.sim.BuildStats;

/** Builds the reference loadout for a gear tier, straight from the shipped gear tables. */
public final class TestBuilds {

    private TestBuilds() {
    }

    /** @param tier 1..6, matching the six zones */
    public static PlayerState playerAtTier(GameContent content, int tier) {
        PlayerState p = new PlayerState();
        for (GearSlot slot : GearSlot.values()) {
            GearTemplate chosen = null;
            for (GearTemplate g : content.gearForSlot(slot)) {
                // Best item in this slot at or below the tier: exactly what a player who has just
                // reached the zone would realistically be wearing.
                if (g.tier <= tier && (chosen == null || g.tier > chosen.tier)) chosen = g;
            }
            if (chosen == null) chosen = content.starterGear(slot);
            String ownedId = "own_" + chosen.id;
            p.gear.put(ownedId, new OwnedGear(ownedId, chosen.id, 1));
            p.loadout.equipped.put(slot, ownedId);
        }
        for (com.vancan.autofishing.sim.SkillDef s : content.skills.values()) {
            if (p.loadout.skills.size() >= Loadout.MAX_SKILLS) break;
            p.loadout.skills.add(s.id);
        }
        return p;
    }

    public static BuildStats buildAtTier(GameContent content, int tier) {
        return LoadoutResolver.resolve(playerAtTier(content, tier), content);
    }
}
