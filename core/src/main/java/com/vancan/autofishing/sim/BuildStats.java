package com.vancan.autofishing.sim;

/**
 * The flattened result of gear + team + talents (GDD 5.2, 8, 9).
 *
 * <p>The simulation never sees an inventory. The meta layer resolves everything a player has
 * equipped into one of these, which means the same simulator runs on the client, on the server
 * during settlement, and in the offline balance harness with no extra plumbing.
 */
public final class BuildStats {

    // Rod
    public float rodPower = 10f;
    public float control = 1f;
    public float critChance = 0.05f;

    // Reel
    public float reelPower = 6f;
    public float pullSpeed = 3.0f;
    public float maxDrag = 60f;
    public float recovery = 1f;

    // Line
    public float lineStrength = 60f;
    public float lineLength = 30f;
    public float elasticity = 1f;

    // Hook
    public float hookRate = 0.88f;
    public float critHook = 0.02f;

    // Float
    public float biteDetection = 1f;
    public float rareDetection = 0f;

    // Bait
    public float attraction = 1f;
    public float rarityBias = 0f;
    public float weightModifier = 1f;
    /** Species id this bait favours, or null. */
    public String speciesBias;

    // Team contribution (GDD 8.1)
    public float teamPull = 0f;
    /** Flat force removed from the tension target; the Controller role's job. */
    public float safetyMitigation = 0f;
    /** Multiplier on damage dealt. Striker. */
    public float damageBonus = 1f;
    /** Reduces how fast the fish drifts away. Anchor. */
    public float distanceControl = 0f;
    /** Multiplier on rare/legendary encounter weight. Hunter. */
    public float luck = 1f;
    /** Multiplier on offline yield. Support. */
    public float offlineEfficiency = 1f;

    /** Highest tension the line tolerates before it snaps outright. */
    public float breakingTension() {
        return Math.max(lineStrength, maxDrag);
    }

    public float safeTension(SimConfig cfg) {
        return breakingTension() * cfg.safeTensionRatio;
    }

    public BuildStats copy() {
        BuildStats b = new BuildStats();
        b.rodPower = rodPower; b.control = control; b.critChance = critChance;
        b.reelPower = reelPower; b.pullSpeed = pullSpeed; b.maxDrag = maxDrag; b.recovery = recovery;
        b.lineStrength = lineStrength; b.lineLength = lineLength; b.elasticity = elasticity;
        b.hookRate = hookRate; b.critHook = critHook;
        b.biteDetection = biteDetection; b.rareDetection = rareDetection;
        b.attraction = attraction; b.rarityBias = rarityBias; b.weightModifier = weightModifier;
        b.speciesBias = speciesBias;
        b.teamPull = teamPull; b.safetyMitigation = safetyMitigation; b.damageBonus = damageBonus;
        b.distanceControl = distanceControl; b.luck = luck; b.offlineEfficiency = offlineEfficiency;
        return b;
    }
}
