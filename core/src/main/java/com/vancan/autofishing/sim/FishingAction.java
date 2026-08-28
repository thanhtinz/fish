package com.vancan.autofishing.sim;

/**
 * One tick of intent, produced either by the Auto controller (GDD 6) or by a human dragging
 * the reel. The simulation only ever consumes this — it has no idea which produced it, which is
 * what lets Auto and manual play share one code path.
 */
public final class FishingAction {

    /** 0 = full slack, 1 = maximum drag. */
    public float pullLevel = 0.6f;
    /** Index into the equipped active skills, or -1 for none. */
    public int skillIndex = -1;
    /** Abort the fight on purpose (Rare Hunter rejecting a low-value catch). */
    public boolean retreat;

    public FishingAction() {
    }

    public FishingAction(float pullLevel) {
        this.pullLevel = pullLevel;
    }

    public FishingAction set(float pullLevel, int skillIndex, boolean retreat) {
        this.pullLevel = pullLevel;
        this.skillIndex = skillIndex;
        this.retreat = retreat;
        return this;
    }
}
