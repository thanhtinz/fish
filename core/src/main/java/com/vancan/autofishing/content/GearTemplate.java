package com.vancan.autofishing.content;

import com.vancan.autofishing.sim.BuildStats;
import com.vancan.autofishing.sim.Rarity;

import java.util.HashMap;
import java.util.Map;

/**
 * An authored gear item (GDD 9). Stats are held as a name->value map rather than as fields so
 * designers can add an affix in JSON without a code change; {@link #apply} is the single place
 * that maps a stat name onto the simulation.
 */
public final class GearTemplate {

    public String id;
    public String name;
    public GearSlot slot = GearSlot.ROD;
    public int tier = 1;
    public Rarity rarity = Rarity.COMMON;
    public String description = "";
    public final Map<String, Float> stats = new HashMap<String, Float>();
    /** Only meaningful for BAIT: the species this bait favours. */
    public String speciesBias;

    /** Upgrading an item scales its numeric stats; level 1 is the printed value. */
    public static float levelScale(int level) {
        return 1f + (level - 1) * 0.11f;
    }

    /** Folds this item's stats into the aggregate build at the given upgrade level. */
    public void apply(BuildStats out, int level) {
        float k = levelScale(level);
        for (Map.Entry<String, Float> e : stats.entrySet()) {
            float v = e.getValue();
            String key = e.getKey();
            // Rates and probabilities are scaled too, but chances are capped when the build is
            // finalised (see LoadoutResolver) rather than here.
            if ("rodPower".equals(key)) out.rodPower += v * k;
            else if ("control".equals(key)) out.control += v * k - 1f;
            else if ("critChance".equals(key)) out.critChance += v * k;
            else if ("reelPower".equals(key)) out.reelPower += v * k;
            else if ("pullSpeed".equals(key)) out.pullSpeed += v * k;
            else if ("maxDrag".equals(key)) out.maxDrag += v * k;
            else if ("recovery".equals(key)) out.recovery += v * k - 1f;
            else if ("lineStrength".equals(key)) out.lineStrength += v * k;
            else if ("lineLength".equals(key)) out.lineLength += v * k;
            else if ("elasticity".equals(key)) out.elasticity += v * k - 1f;
            else if ("hookRate".equals(key)) out.hookRate += v * k;
            else if ("critHook".equals(key)) out.critHook += v * k;
            else if ("biteDetection".equals(key)) out.biteDetection += v * k - 1f;
            else if ("rareDetection".equals(key)) out.rareDetection += v * k;
            else if ("attraction".equals(key)) out.attraction += v * k - 1f;
            else if ("rarityBias".equals(key)) out.rarityBias += v * k;
            else if ("weightModifier".equals(key)) out.weightModifier += v * k - 1f;
            else throw new IllegalStateException(
                    "Gear '" + id + "' declares unknown stat '" + key + "'");
        }
        if (slot == GearSlot.BAIT && speciesBias != null) {
            out.speciesBias = speciesBias;
        }
    }

    /** Base sell/upgrade cost, driven by tier and rarity so the economy stays predictable. */
    public int upgradeCost(int currentLevel) {
        return (int) (45 * Math.pow(1.9, tier - 1) * Math.pow(1.28, currentLevel - 1)
                * rarity.valueMultiplier * 0.35f);
    }
}
