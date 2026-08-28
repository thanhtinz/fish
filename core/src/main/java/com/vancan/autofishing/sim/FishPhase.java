package com.vancan.autofishing.sim;

/**
 * Phases a hooked fish moves through. The phase is what makes two fish of the same weight
 * feel different to fight (GDD 7) and it is the main input the Auto controller reacts to.
 */
public enum FishPhase {
    /** Steady pull. The baseline. */
    STEADY("Ổn định", 1.00f, 1.00f),
    /** Sprints away: distance climbs fast, force spikes. */
    RUN("Bỏ chạy", 1.25f, 1.80f),
    /** Dives: heavy force, little lateral movement, tension trap. */
    DIVE("Lặn sâu", 1.55f, 0.45f),
    /** Rage window: dangerous but burns the fish's own stamina. */
    RAGE("Cuồng nộ", 1.85f, 1.40f),
    /** Feint used by Tricksters: looks exhausted, then snaps back. */
    FEINT("Giả vờ", 0.35f, 0.30f),
    /** Genuinely exhausted: the window the Auto controller should burst in. */
    TIRED("Kiệt sức", 0.55f, 0.35f);

    public final String displayName;
    /** Multiplier on the force the fish applies to the line. */
    public final float forceMultiplier;
    /** Multiplier on how hard the fish drives away from the angler. */
    public final float driveMultiplier;

    FishPhase(String displayName, float forceMultiplier, float driveMultiplier) {
        this.displayName = displayName;
        this.forceMultiplier = forceMultiplier;
        this.driveMultiplier = driveMultiplier;
    }

    /** True while the phase is a burst window worth spending cooldowns on. */
    public boolean isOpening() {
        return this == TIRED || this == RAGE;
    }
}
