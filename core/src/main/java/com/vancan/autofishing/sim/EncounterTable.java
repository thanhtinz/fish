package com.vancan.autofishing.sim;

import java.util.ArrayList;
import java.util.List;

/**
 * Weighted fish pool for one spot (GDD 10). Bait and the Hunter role bend the weights towards
 * rarer entries rather than adding a separate "rare roll", so luck stays a single readable number.
 */
public final class EncounterTable {

    public static final class Entry {
        public final SpeciesDef species;
        public final float weight;

        public Entry(SpeciesDef species, float weight) {
            this.species = species;
            this.weight = weight;
        }
    }

    private final List<Entry> entries = new ArrayList<Entry>();

    public void add(SpeciesDef species, float weight) {
        entries.add(new Entry(species, weight));
    }

    public List<Entry> getEntries() {
        return entries;
    }

    public boolean isEmpty() {
        return entries.isEmpty();
    }

    /**
     * Rolls one species.
     *
     * @param build supplies luck, rarity bias and any species bias from the equipped bait
     */
    public SpeciesDef roll(Rng rng, BuildStats build) {
        if (entries.isEmpty()) throw new IllegalStateException("Encounter table is empty");

        float total = 0f;
        float[] weights = new float[entries.size()];
        for (int i = 0; i < entries.size(); i++) {
            Entry e = entries.get(i);
            float w = e.weight * rarityBoost(e.species.rarity, build);
            if (build.speciesBias != null && build.speciesBias.equals(e.species.id)) {
                w *= 3.5f;
            }
            weights[i] = w;
            total += w;
        }

        double pick = rng.nextDouble() * total;
        for (int i = 0; i < weights.length; i++) {
            pick -= weights[i];
            if (pick <= 0) return entries.get(i).species;
        }
        return entries.get(entries.size() - 1).species;
    }

    /**
     * Rarity boost is exponential in the rarity tier so that a small amount of luck is felt at
     * the top of the table without flooding the player with commons-turned-legendaries.
     */
    private float rarityBoost(Rarity rarity, BuildStats build) {
        int tier = rarity.ordinal();
        if (tier == 0) return 1f;
        float bias = build.luck * (1f + build.rarityBias);
        return (float) Math.pow(bias, tier * 0.55f);
    }

    /** Expected value of one catch, used by the offline settlement model (GDD 12). */
    public float expectedRarityMultiplier(BuildStats build) {
        float total = 0f, acc = 0f;
        for (Entry e : entries) {
            float w = e.weight * rarityBoost(e.species.rarity, build);
            total += w;
            acc += w * e.species.rarity.valueMultiplier;
        }
        return total <= 0 ? 1f : acc / total;
    }
}
