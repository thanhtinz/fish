package com.vancan.autofishing.meta;

import com.vancan.autofishing.sim.Rarity;

/** A landed fish (GDD 11.1). Feeds the codex, the economy and the tournament scores. */
public final class FishRecord {
    public String speciesId;
    public float weight;
    public Rarity rarity = Rarity.COMMON;
    public String spotId;
    public long caughtAt;
    public int goldValue;
    public int xpValue;
    /** True when this beat the player's previous best for the species. */
    public boolean personalBest;
}
