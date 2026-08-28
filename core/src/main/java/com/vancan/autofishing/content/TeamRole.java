package com.vancan.autofishing.content;

/** GDD 8.1. Each role maps onto exactly one lever of the simulation, so a formation reads clearly. */
public enum TeamRole {
    ANCHOR("Trấn", "Giảm tốc độ cá kéo xa"),
    STRIKER("Kích", "Tăng sát thương và lực kéo"),
    CONTROLLER("Khống", "Giảm lực căng, chống sốc đổi pha"),
    HUNTER("Săn", "Tăng cơ hội gặp cá hiếm"),
    SUPPORT("Trợ", "Hồi phục và hiệu suất ngoại tuyến");

    public final String displayName;
    public final String summary;

    TeamRole(String displayName, String summary) {
        this.displayName = displayName;
        this.summary = summary;
    }

    public static TeamRole fromId(String id) {
        for (TeamRole r : values()) if (r.name().equalsIgnoreCase(id)) return r;
        throw new IllegalArgumentException("Unknown team role: " + id);
    }
}
