package com.vancan.autofishing.meta;

import com.vancan.autofishing.sim.FishState;
import com.vancan.autofishing.sim.SpeciesDef;

/** Turns a landed fish into gold, XP and a codex record (GDD 11). */
public final class CatchResolver {

    private CatchResolver() {
    }

    /**
     * Weight scales value sub-linearly. A linear curve would make one lucky record catch worth
     * more than an hour of play and wreck the sink/source model in GDD 23.
     */
    public static float weightMultiplier(FishState fish) {
        float relative = fish.weight / Math.max(0.01f, fish.species.minWeight);
        return (float) (1.0 + 0.55 * Math.pow(relative, 0.62));
    }

    public static FishRecord resolve(FishState fish, String spotId, long now) {
        SpeciesDef s = fish.species;
        float mul = weightMultiplier(fish) * s.rarity.valueMultiplier;

        FishRecord r = new FishRecord();
        r.speciesId = s.id;
        r.weight = fish.weight;
        r.rarity = s.rarity;
        r.spotId = spotId;
        r.caughtAt = now;
        r.goldValue = Math.max(1, (int) (s.baseValue * mul));
        r.xpValue = Math.max(1, (int) (s.baseXp * (1f + 0.35f * (mul - 1f))));
        return r;
    }
}
