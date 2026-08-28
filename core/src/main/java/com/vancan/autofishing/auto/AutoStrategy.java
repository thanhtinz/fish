package com.vancan.autofishing.auto;

import com.vancan.autofishing.sim.Rarity;

/** The five tactical presets from GDD 6. */
public enum AutoStrategy {

    BALANCED("Cân bằng", 0.65f, 0.50f, null, 0f, "Ổn định mọi mặt",
            "Giữ lực kéo vừa phải, an toàn trong hầu hết tình huống."),

    AGGRESSIVE("Áp đảo", 0.90f, 0.85f, null, 0f, "Nhanh nhất, dễ đứt dây",
            "Kéo mạnh, chấp nhận rủi ro đứt dây. Dùng cho boss hoặc khi thừa chỉ số."),

    SAFE("An toàn", 0.46f, 0.20f, null, 0f, "Ít mất cá, chậm hơn",
            "Ưu tiên giữ dây. Chậm nhưng ít mất cá khi ngư cụ còn yếu."),

    RARE_HUNTER("Săn hiếm", 0.65f, 0.50f, Rarity.RARE, 0f, "Chỉ đánh cá hiếm trở lên",
            "Bỏ qua cá phổ thông để tiết kiệm thời gian, tập trung cá hiếm trở lên."),

    HEAVY_HUNTER("Săn cân", 0.75f, 0.65f, null, 12f, "Chỉ đánh cá trên 12 kg",
            "Chỉ đánh cá vượt ngưỡng trọng lượng, bỏ cá nhỏ.");

    public final String displayName;
    /** Base pull level this policy aims for in a neutral phase. */
    public final float pullBias;
    /** 0..1. How close to the breaking point the policy is willing to sit. */
    public final float riskTolerance;
    /** Minimum rarity worth fighting, or null to accept everything. */
    public final Rarity minRarity;
    /** Minimum weight in kg worth fighting, or 0 for no filter. */
    public final float minWeight;
    /** Few-word label for the fishing HUD, where there is only one line of room. */
    public final String summary;
    public final String description;

    AutoStrategy(String displayName, float pullBias, float riskTolerance,
                 Rarity minRarity, float minWeight, String summary, String description) {
        this.displayName = displayName;
        this.pullBias = pullBias;
        this.riskTolerance = riskTolerance;
        this.minRarity = minRarity;
        this.minWeight = minWeight;
        this.summary = summary;
        this.description = description;
    }
}
