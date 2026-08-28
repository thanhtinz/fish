package com.vancan.autofishing.sim;

/**
 * Every tunable coefficient of the fishing simulation.
 *
 * <p>GDD 5.3: balance coefficients must not be hard-coded in the client. This object is loaded
 * from {@code assets/data/sim_config.json} and is meant to be overridden by server Remote Config,
 * so a rebalance is a config push rather than a client release.
 */
public final class SimConfig {

    /** Fixed simulation step. The sim is never advanced with a variable frame delta. */
    public float tickSeconds = 1f / 30f;

    // --- Encounter -----------------------------------------------------------------------
    public float baseBiteIntervalSeconds = 4.5f;
    public float biteWindowSeconds = 1.6f;
    public float searchTimeoutSeconds = 45f;

    // --- Pull / damage -------------------------------------------------------------------
    public float damageCoefficient = 1.10f;
    public float critMultiplier = 1.8f;
    public float staminaDamageCoefficient = 1.05f;
    /**
     * How much of the angler's own pull shows up as line tension. This is the single most
     * important number in the game: it is the price the player pays for damage, and if it is low
     * the Auto strategies collapse into each other because backing off stops being a real choice.
     */
    public float pullTensionCoefficient = 1.70f;

    // --- Tension / line ------------------------------------------------------------------
    /** Fraction of line strength above which the line starts taking permanent wear. */
    public float safeTensionRatio = 0.70f;
    /**
     * Wear coefficient. Wear is <em>quadratic</em> in how far tension exceeds the safe threshold,
     * not linear.
     *
     * <p>Under a linear model the Safe policy snapped as many lines as the Aggressive one: its
     * small, occasional overshoots during phase shocks cost little per second, but its fights ran
     * 60% longer and the totals came out even. Squaring the overshoot makes brushing the limit
     * nearly free and riding it genuinely expensive, which is the risk gradient the five presets
     * are supposed to trade along. It also matches how material fatigue actually behaves.
     */
    public float wearRate = 1.25f;
    /** How fast tension chases its target. Higher = twitchier, less forgiving. */
    public float lineResponse = 2.6f;
    public float elasticityRelief = 0.30f;
    /**
     * Instant tension shock when a fish changes phase, as a fraction of the force jump.
     *
     * <p>Without it, tension only ever drifts towards its target and an Aggressive policy parked
     * just under the breaking point is never actually punished — it out-earns every other preset
     * with no downside. The shock is what makes a thin safety margin a real gamble and what gives
     * the Controller role and Brace skills something to answer.
     */
    public float phaseShockFactor = 0.85f;

    // --- Distance ------------------------------------------------------------------------
    public float landingThreshold = 1.5f;
    /**
     * Fraction of the line already paid out when the hook sets. Starting a fight at distance zero
     * made the spool a dead mechanic and left the Anchor role with nothing to do, so a cast now
     * begins well out and reeling in is a real half of the fight.
     */
    public float castDistanceRatio = 0.42f;
    public float reelSpeedCoefficient = 1.0f;

    // --- Fight flow ----------------------------------------------------------------------
    public float fightTimeoutSeconds = 90f;
    /** Stamina regenerates while the player gives slack, which is what makes stalling costly. */
    public float fishStaminaRecovery = 0.055f;
    public float fatigueForceFloor = 0.35f;

    // --- Offline / idle (GDD 12) ---------------------------------------------------------
    public float offlinePenalty = 0.55f;
    public float offlineCapHours = 8f;

    public SimConfig copy() {
        SimConfig c = new SimConfig();
        c.tickSeconds = tickSeconds;
        c.baseBiteIntervalSeconds = baseBiteIntervalSeconds;
        c.biteWindowSeconds = biteWindowSeconds;
        c.searchTimeoutSeconds = searchTimeoutSeconds;
        c.damageCoefficient = damageCoefficient;
        c.critMultiplier = critMultiplier;
        c.staminaDamageCoefficient = staminaDamageCoefficient;
        c.pullTensionCoefficient = pullTensionCoefficient;
        c.safeTensionRatio = safeTensionRatio;
        c.wearRate = wearRate;
        c.lineResponse = lineResponse;
        c.elasticityRelief = elasticityRelief;
        c.phaseShockFactor = phaseShockFactor;
        c.landingThreshold = landingThreshold;
        c.castDistanceRatio = castDistanceRatio;
        c.reelSpeedCoefficient = reelSpeedCoefficient;
        c.fightTimeoutSeconds = fightTimeoutSeconds;
        c.fishStaminaRecovery = fishStaminaRecovery;
        c.fatigueForceFloor = fatigueForceFloor;
        c.offlinePenalty = offlinePenalty;
        c.offlineCapHours = offlineCapHours;
        return c;
    }
}
