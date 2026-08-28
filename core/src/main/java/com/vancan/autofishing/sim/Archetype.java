package com.vancan.autofishing.sim;

/**
 * Behaviour families from GDD 7. The archetype owns the phase machine: given the fish's
 * current condition it decides which phase to move to and how long it lasts. Species reference
 * an archetype and then scale it with their own stats, so a Runner minnow and a Runner marlin
 * share a rhythm but not a difficulty.
 */
public enum Archetype {

    /** Fast, distance climbs, frequent short sprints. */
    RUNNER("Cá chạy") {
        @Override
        public FishPhase nextPhase(FishState f, Rng rng) {
            if (f.staminaRatio() < 0.18f) return FishPhase.TIRED;
            return rng.chance(0.55) ? FishPhase.RUN : FishPhase.STEADY;
        }

        @Override
        public float phaseDuration(FishPhase phase, Rng rng) {
            return phase == FishPhase.RUN ? (float) rng.range(1.4, 2.6) : (float) rng.range(2.0, 3.4);
        }
    },

    /** Slow but enormously strong: long high-tension windows. */
    POWER_TANK("Cá lì") {
        @Override
        public FishPhase nextPhase(FishState f, Rng rng) {
            if (f.staminaRatio() < 0.14f) return FishPhase.TIRED;
            return rng.chance(0.45) ? FishPhase.DIVE : FishPhase.STEADY;
        }

        @Override
        public float phaseDuration(FishPhase phase, Rng rng) {
            return phase == FishPhase.DIVE ? (float) rng.range(3.0, 5.0) : (float) rng.range(2.5, 4.0);
        }
    },

    /** Direction changes produce short, sharp tension spikes. */
    ERRATIC("Cá loạn") {
        @Override
        public FishPhase nextPhase(FishState f, Rng rng) {
            if (f.staminaRatio() < 0.20f) return FishPhase.TIRED;
            double r = rng.nextDouble();
            if (r < 0.34) return FishPhase.RUN;
            if (r < 0.62) return FishPhase.RAGE;
            return FishPhase.STEADY;
        }

        @Override
        public float phaseDuration(FishPhase phase, Rng rng) {
            return (float) rng.range(0.8, 1.8);
        }
    },

    /** Lives deep: dives punish distance, and it rarely tires on its own. */
    DIVER("Cá lặn") {
        @Override
        public FishPhase nextPhase(FishState f, Rng rng) {
            if (f.staminaRatio() < 0.12f) return FishPhase.TIRED;
            return rng.chance(0.62) ? FishPhase.DIVE : FishPhase.STEADY;
        }

        @Override
        public float phaseDuration(FishPhase phase, Rng rng) {
            return phase == FishPhase.DIVE ? (float) rng.range(3.5, 6.0) : (float) rng.range(1.8, 3.0);
        }
    },

    /** Fakes exhaustion. Punishes an Auto policy that only reads the stamina bar. */
    TRICKSTER("Cá ranh") {
        @Override
        public FishPhase nextPhase(FishState f, Rng rng) {
            if (f.staminaRatio() < 0.10f) return FishPhase.TIRED;
            double r = rng.nextDouble();
            if (r < 0.30) return FishPhase.FEINT;
            if (r < 0.50) return FishPhase.RAGE;
            if (r < 0.70) return FishPhase.RUN;
            return FishPhase.STEADY;
        }

        @Override
        public float phaseDuration(FishPhase phase, Rng rng) {
            return phase == FishPhase.FEINT ? (float) rng.range(1.2, 2.2) : (float) rng.range(1.5, 2.8);
        }
    },

    /** Multi-phase, scripted rotation. Bosses do not roll randomly (GDD 7). */
    BOSS("Thủy quái") {
        @Override
        public FishPhase nextPhase(FishState f, Rng rng) {
            if (f.staminaRatio() < 0.08f) return FishPhase.TIRED;
            // Deterministic rotation keyed off how far the boss has been worn down, so every
            // player meets the same fight structure and can build against it.
            int step = f.phaseIndex % 4;
            switch (step) {
                case 0: return FishPhase.STEADY;
                case 1: return FishPhase.DIVE;
                case 2: return f.hpRatio() < 0.5f ? FishPhase.RAGE : FishPhase.RUN;
                default: return FishPhase.RAGE;
            }
        }

        @Override
        public float phaseDuration(FishPhase phase, Rng rng) {
            return phase == FishPhase.RAGE ? 4.5f : 3.5f;
        }
    };

    public final String displayName;

    Archetype(String displayName) {
        this.displayName = displayName;
    }

    /** Chooses the phase to enter once the current one expires. */
    public abstract FishPhase nextPhase(FishState f, Rng rng);

    /** How long the chosen phase runs, in seconds. */
    public abstract float phaseDuration(FishPhase phase, Rng rng);

    public static Archetype fromId(String id) {
        for (Archetype a : values()) {
            if (a.name().equalsIgnoreCase(id)) return a;
        }
        throw new IllegalArgumentException("Unknown archetype: " + id);
    }
}
