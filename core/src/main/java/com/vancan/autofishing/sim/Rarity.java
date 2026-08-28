package com.vancan.autofishing.sim;

/** GDD 11.2. Ordinal order is meaningful: higher ordinal == rarer. */
public enum Rarity {
    COMMON("Thường", 1.0f),
    UNCOMMON("Khá", 1.8f),
    RARE("Hiếm", 3.6f),
    EPIC("Sử thi", 8.0f),
    LEGENDARY("Huyền thoại", 20f),
    MYTHIC("Thần thoại", 55f),
    SECRET("Bí ẩn", 150f);

    public final String displayName;
    /** Multiplier applied to gold/XP value of a catch. */
    public final float valueMultiplier;

    Rarity(String displayName, float valueMultiplier) {
        this.displayName = displayName;
        this.valueMultiplier = valueMultiplier;
    }

    public static Rarity fromId(String id) {
        for (Rarity r : values()) {
            if (r.name().equalsIgnoreCase(id)) return r;
        }
        throw new IllegalArgumentException("Unknown rarity: " + id);
    }
}
