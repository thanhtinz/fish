package com.vancan.autofishing.content;

import com.vancan.autofishing.sim.BuildStats;
import com.vancan.autofishing.sim.Rarity;

import java.util.HashMap;
import java.util.Map;

/** A recruitable angler (GDD 8). Contribution scales linearly with level and star rank. */
public final class AnglerDef {

    public String id;
    public String name;
    public TeamRole role = TeamRole.STRIKER;
    public Rarity rarity = Rarity.COMMON;
    public String signatureSkill;
    public String description = "";
    /** Contribution per level; the captain slot gets a bonus multiplier. */
    public final Map<String, Float> statsPerLevel = new HashMap<String, Float>();

    public static float starScale(int stars) {
        return 1f + (stars - 1) * 0.18f;
    }

    /**
     * @param captainBonus 1.0 for a support slot, higher for the captain slot
     */
    public void apply(BuildStats out, int level, int stars, float captainBonus) {
        float k = level * starScale(stars) * captainBonus;
        for (Map.Entry<String, Float> e : statsPerLevel.entrySet()) {
            float v = e.getValue() * k;
            String key = e.getKey();
            if ("teamPull".equals(key)) out.teamPull += v;
            else if ("safetyMitigation".equals(key)) out.safetyMitigation += v;
            else if ("damageBonus".equals(key)) out.damageBonus += v;
            else if ("distanceControl".equals(key)) out.distanceControl += v;
            else if ("luck".equals(key)) out.luck += v;
            else if ("offlineEfficiency".equals(key)) out.offlineEfficiency += v;
            else if ("critChance".equals(key)) out.critChance += v;
            else if ("recovery".equals(key)) out.recovery += v;
            else if ("elasticity".equals(key)) out.elasticity += v;
            else if ("rareDetection".equals(key)) out.rareDetection += v;
            else throw new IllegalStateException(
                    "Angler '" + id + "' declares unknown stat '" + key + "'");
        }
    }
}
