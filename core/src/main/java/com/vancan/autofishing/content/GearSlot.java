package com.vancan.autofishing.content;

/** GDD 9. */
public enum GearSlot {
    ROD("Cần"), REEL("Máy"), LINE("Dây"), HOOK("Lưỡi"), FLOAT("Phao"), BAIT("Mồi");

    public final String displayName;

    GearSlot(String displayName) {
        this.displayName = displayName;
    }

    public static GearSlot fromId(String id) {
        for (GearSlot s : values()) if (s.name().equalsIgnoreCase(id)) return s;
        throw new IllegalArgumentException("Unknown gear slot: " + id);
    }
}
