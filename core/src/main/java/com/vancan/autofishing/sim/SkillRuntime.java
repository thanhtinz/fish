package com.vancan.autofishing.sim;

/** Per-session cooldown/uptime tracking for one equipped skill. */
public final class SkillRuntime {

    public final SkillDef def;
    public float cooldownRemaining;
    public float activeRemaining;

    public SkillRuntime(SkillDef def) {
        this.def = def;
    }

    public boolean isReady() {
        return cooldownRemaining <= 0f && activeRemaining <= 0f;
    }

    public boolean isActive() {
        return activeRemaining > 0f;
    }

    public void trigger() {
        activeRemaining = def.durationSeconds;
        cooldownRemaining = def.cooldownSeconds;
    }

    public void tick(float dt) {
        if (activeRemaining > 0f) activeRemaining -= dt;
        if (cooldownRemaining > 0f) cooldownRemaining -= dt;
    }

    public float cooldownRatio() {
        return def.cooldownSeconds <= 0 ? 0 : Math.max(0f, cooldownRemaining) / def.cooldownSeconds;
    }
}
