package com.vancan.autofishing.ui;

import com.badlogic.gdx.graphics.Color;
import com.vancan.autofishing.sim.Rarity;

/** One place for every colour and metric, so the vertical layout stays consistent. */
public final class Theme {

    private Theme() {
    }

    /** Design resolution. Everything is authored against this and scaled by the viewport. */
    public static final float WORLD_WIDTH = 1080f;
    public static final float WORLD_HEIGHT = 1920f;

    public static final Color SKY_TOP = new Color(0.06f, 0.10f, 0.20f, 1f);
    public static final Color SKY_BOTTOM = new Color(0.13f, 0.28f, 0.42f, 1f);
    public static final Color WATER_TOP = new Color(0.08f, 0.32f, 0.44f, 1f);
    public static final Color WATER_DEEP = new Color(0.02f, 0.09f, 0.18f, 1f);

    public static final Color PANEL = new Color(0.07f, 0.10f, 0.15f, 0.90f);
    public static final Color PANEL_LIGHT = new Color(0.13f, 0.18f, 0.25f, 0.95f);
    public static final Color BORDER = new Color(0.30f, 0.45f, 0.58f, 1f);

    public static final Color TEXT = new Color(0.93f, 0.96f, 0.99f, 1f);
    public static final Color TEXT_DIM = new Color(0.62f, 0.70f, 0.78f, 1f);
    public static final Color ACCENT = new Color(0.25f, 0.78f, 0.92f, 1f);
    public static final Color GOLD = new Color(1.00f, 0.79f, 0.28f, 1f);

    public static final Color GOOD = new Color(0.36f, 0.84f, 0.52f, 1f);
    public static final Color WARN = new Color(0.98f, 0.74f, 0.24f, 1f);
    public static final Color DANGER = new Color(0.94f, 0.35f, 0.35f, 1f);

    public static final Color TENSION_SAFE = new Color(0.36f, 0.84f, 0.52f, 1f);
    public static final Color TENSION_WARN = new Color(0.98f, 0.74f, 0.24f, 1f);
    public static final Color TENSION_DANGER = new Color(0.94f, 0.29f, 0.29f, 1f);

    public static final Color BUTTON = new Color(0.16f, 0.24f, 0.34f, 1f);
    public static final Color BUTTON_ACTIVE = new Color(0.20f, 0.52f, 0.68f, 1f);
    public static final Color BUTTON_DOWN = new Color(0.11f, 0.17f, 0.24f, 1f);
    public static final Color BUTTON_DISABLED = new Color(0.14f, 0.16f, 0.19f, 1f);

    /** Minimum touch target. Anything smaller fails the one-handed test in GDD 23. */
    public static final float TOUCH_MIN = 96f;
    public static final float PAD = 24f;

    private static final Color[] RARITY_COLORS = {
            new Color(0.72f, 0.76f, 0.80f, 1f), // Common
            new Color(0.44f, 0.82f, 0.50f, 1f), // Uncommon
            new Color(0.33f, 0.62f, 0.96f, 1f), // Rare
            new Color(0.70f, 0.45f, 0.95f, 1f), // Epic
            new Color(1.00f, 0.68f, 0.22f, 1f), // Legendary
            new Color(0.98f, 0.35f, 0.45f, 1f), // Mythic
            new Color(0.40f, 0.98f, 0.90f, 1f), // Secret
    };

    public static Color rarityColor(Rarity rarity) {
        return RARITY_COLORS[rarity.ordinal()];
    }

    /** Green below the wear line, amber in the wear band, red near the breaking point. */
    public static Color tensionColor(float ratio, float safeRatio) {
        if (ratio < safeRatio) return TENSION_SAFE;
        if (ratio < safeRatio + (1f - safeRatio) * 0.6f) return TENSION_WARN;
        return TENSION_DANGER;
    }
}
