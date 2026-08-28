package com.vancan.autofishing.sim;

/**
 * Static definition of a fish species, authored in {@code assets/data/species.json}.
 * Nothing here changes at runtime; the per-catch numbers live in {@link FishState}.
 */
public final class SpeciesDef {

    public String id;
    public String name;
    public Rarity rarity = Rarity.COMMON;
    public Archetype archetype = Archetype.RUNNER;

    /** Weight band in kg. The roll inside the band is biased by {@link #weightBias}. */
    public float minWeight = 0.5f;
    public float maxWeight = 3f;
    /** >1 pushes rolls towards the light end, so a record-weight catch stays an event. */
    public float weightBias = 2.4f;

    /** Per-kg stat scaling. Final stats are base + perKg * weight (see FishState.of). */
    public float baseHp = 40f;
    public float hpPerKg = 9f;
    public float baseStamina = 30f;
    public float staminaPerKg = 5.5f;
    public float basePower = 12f;
    public float powerPerKg = 1.9f;
    public float baseSpeed = 2.4f;
    public float speedPerKg = 0.045f;

    /** Chance per bite that the fish refuses the hook outright. */
    public float escapeRate = 0.05f;
    /** How hard the float is to read for this species; feeds hook success. */
    public float hookDifficulty = 1f;

    /** Preferred depth band, matched against the spot and the float. Cosmetic + codex filter. */
    public float depth = 1f;

    /** Base gold before rarity and weight multipliers. */
    public int baseValue = 10;
    public int baseXp = 6;

    public String description = "";

    public boolean isBoss() {
        return archetype == Archetype.BOSS;
    }
}
