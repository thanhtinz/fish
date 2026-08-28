package com.vancan.autofishing.sim;

/**
 * The live, mutable fish being fought (GDD 5.1). One instance per encounter.
 */
public final class FishState {

    public SpeciesDef species;

    public float weight;
    public Rarity rarity;

    public float maxHp;
    public float hp;
    public float maxStamina;
    public float stamina;
    public float power;
    public float speed;

    public FishPhase phase = FishPhase.STEADY;
    /** Seconds left in the current phase. */
    public float phaseTimer;
    /** Monotonic counter of phase changes; boss rotations key off it. */
    public int phaseIndex;

    /** True once the feint has been used, so a Trickster cannot loop it forever. */
    public boolean feintSpent;

    /** Rolls the concrete fish a player just hooked. */
    public static FishState of(SpeciesDef species, Rng rng) {
        FishState f = new FishState();
        f.species = species;
        f.rarity = species.rarity;
        f.weight = (float) rng.weighted(species.minWeight, species.maxWeight, species.weightBias);

        f.maxHp = species.baseHp + species.hpPerKg * f.weight;
        f.maxStamina = species.baseStamina + species.staminaPerKg * f.weight;
        f.power = species.basePower + species.powerPerKg * f.weight;
        f.speed = species.baseSpeed + species.speedPerKg * f.weight;

        f.hp = f.maxHp;
        f.stamina = f.maxStamina;
        f.phase = FishPhase.STEADY;
        f.phaseTimer = species.archetype.phaseDuration(FishPhase.STEADY, rng);
        return f;
    }

    public float hpRatio() {
        return maxHp <= 0 ? 0 : hp / maxHp;
    }

    public float staminaRatio() {
        return maxStamina <= 0 ? 0 : stamina / maxStamina;
    }

    /**
     * Force falls off as the fish tires, but never to zero: a big fish stays dangerous even
     * when spent, which is what keeps the last stretch of a fight tense.
     */
    public float currentForce(SimConfig cfg) {
        float fatigue = cfg.fatigueForceFloor + (1f - cfg.fatigueForceFloor) * staminaRatio();
        return power * phase.forceMultiplier * fatigue;
    }

    public float currentDrive() {
        return speed * phase.driveMultiplier * (0.45f + 0.55f * staminaRatio());
    }

    /** Advances the phase machine. Called once per tick by the session. */
    public void tickPhase(float dt, Rng rng) {
        phaseTimer -= dt;
        if (phaseTimer > 0) return;

        FishPhase next = species.archetype.nextPhase(this, rng);
        if (next == FishPhase.FEINT) {
            if (feintSpent) {
                next = FishPhase.RAGE;
            } else {
                feintSpent = true;
            }
        }
        phase = next;
        phaseIndex++;
        phaseTimer = species.archetype.phaseDuration(next, rng);
    }
}
