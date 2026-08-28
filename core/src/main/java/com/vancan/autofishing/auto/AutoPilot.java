package com.vancan.autofishing.auto;

import com.vancan.autofishing.sim.FishPhase;
import com.vancan.autofishing.sim.FishState;
import com.vancan.autofishing.sim.FishingAction;
import com.vancan.autofishing.sim.FishingSession;
import com.vancan.autofishing.sim.SessionPhase;
import com.vancan.autofishing.sim.SkillDef;
import com.vancan.autofishing.sim.SkillRuntime;

/**
 * The Auto Fishing controller (GDD 6).
 *
 * <p>This is deliberately a <em>policy over the simulation</em> rather than a shortcut around it:
 * it observes the same state the HUD shows and emits the same {@link FishingAction} a human thumb
 * would. Auto therefore cannot out-earn what the player's build can actually land, which is the
 * whole reason the mode is not just an idle reward tap (Pillar "Auto != idle-only").
 *
 * <p>It holds no randomness of its own, so an Auto session stays fully replayable from its seed.
 */
public final class AutoPilot {

    private AutoStrategy strategy;
    /** Reused every tick; the fight loop runs at 30Hz and should not allocate. */
    private final FishingAction action = new FishingAction();

    public AutoPilot(AutoStrategy strategy) {
        this.strategy = strategy;
    }

    public void setStrategy(AutoStrategy strategy) {
        this.strategy = strategy;
    }

    public AutoStrategy getStrategy() {
        return strategy;
    }

    /** Produces the action for the current tick. */
    public FishingAction decide(FishingSession session) {
        action.set(strategy.pullBias, -1, false);

        if (session.getPhase() == SessionPhase.SEARCHING) {
            // Nothing to steer yet; keep a light pull so the hook-set trigger fires the moment
            // the bite lands rather than a tick later.
            action.pullLevel = 0.3f;
            return action;
        }

        if (session.getPhase() == SessionPhase.BITE) {
            action.pullLevel = 1f; // strike immediately: the window shrinks every tick
            return action;
        }

        if (session.getPhase() != SessionPhase.FIGHT) {
            return action;
        }

        FishState fish = session.getFish();

        // --- Target filter: abort fish this policy does not want (GDD 6.1) ----------------
        if (shouldAbort(fish)) {
            action.retreat = true;
            return action;
        }

        float tensionRatio = session.getTensionRatio();
        float integrity = session.getLineIntegrity();
        float distanceRatio = session.getDistanceRatio();

        // The danger line is where the policy stops pushing.
        //
        // The Safe policy deliberately targets *below* the wear threshold, so it takes no line
        // damage at all in a neutral phase and pays for that in fight length and spool risk
        // instead. Aggressive rides just under the hard breaking point. Anchoring the band to the
        // configured wear threshold keeps the five presets meaningful after a rebalance.
        float safeLine = session.getSafeTensionRatio();
        float floor = safeLine * 0.88f;
        float ceiling = 0.94f;
        float danger = floor + (ceiling - floor) * strategy.riskTolerance;

        float pull = strategy.pullBias;

        // Rule 1: over the danger line, back off proportionally to the overshoot.
        if (tensionRatio > danger) {
            float overshoot = (tensionRatio - danger) / Math.max(0.05f, 1f - danger);
            pull *= Math.max(0.1f, 1f - overshoot * 1.6f);
        }

        // Rule 2: the fish is spent — this is the window, spend it.
        if (fish.staminaRatio() < 0.25f && tensionRatio < danger) {
            pull = Math.min(1f, pull * 1.45f);
        }

        // Rule 3: a failing line outranks DPS. Survival first.
        if (integrity < 0.35f) {
            pull = Math.min(pull, 0.35f + 0.3f * strategy.riskTolerance);
        }

        // Rule 4: react to the phase rather than to the average.
        pull *= phaseAdjustment(fish.phase, tensionRatio, danger);

        // Rule 5: being spooled loses the fish outright, so distance beats caution near the end
        // of the line — a snapped line and an emptied spool cost the same fish.
        if (distanceRatio > 0.68f) {
            pull = Math.max(pull, 0.50f + 0.45f * distanceRatio);
        }

        action.pullLevel = clamp01(pull);
        action.skillIndex = chooseSkill(session, fish, tensionRatio, integrity, danger);
        return action;
    }

    /**
     * Phase-specific bias. FEINT is the interesting one: the fish looks exhausted, so a naive
     * stamina-only policy would burst into a spike. Reading the phase is what makes Tricksters a
     * build check instead of a coin flip.
     */
    private float phaseAdjustment(FishPhase phase, float tensionRatio, float danger) {
        switch (phase) {
            case RAGE:
                // Rage burns the fish's own stamina; ride it if there is headroom, duck if not.
                return tensionRatio < danger * 0.8f ? 1.10f : 0.65f;
            case DIVE:
                return 0.72f;   // dives are the classic line-snap trap
            case RUN:
                return 0.92f;   // let it run a little, but do not hand it the spool
            case TIRED:
                return 1.35f;
            case FEINT:
                return 0.80f;   // deliberately do NOT burst
            case STEADY:
            default:
                return 1f;
        }
    }

    private int chooseSkill(FishingSession session, FishState fish,
                            float tensionRatio, float integrity, float danger) {
        SkillRuntime[] skills = session.getSkills();
        int best = -1;
        int bestPriority = 0;

        for (int i = 0; i < skills.length; i++) {
            SkillRuntime s = skills[i];
            if (!s.isReady()) continue;

            int priority = 0;
            switch (s.def.effect) {
                case BRACE:
                    // Emergency use only, or the cooldown is wasted on a spike that never came.
                    if (tensionRatio > danger * 0.95f) priority = 100;
                    break;
                case MEND:
                    if (integrity < 0.30f) priority = 90;
                    break;
                case BURST:
                    if (fish.phase.isOpening() && tensionRatio < danger) priority = 70;
                    else if (fish.staminaRatio() < 0.3f && tensionRatio < danger) priority = 60;
                    break;
                case WINCH:
                    if (session.getDistanceRatio() > 0.6f) priority = 80;
                    else if (fish.staminaRatio() <= 0f) priority = 65;
                    break;
                case EXHAUST:
                    // Best value early, while there is still stamina left to strip.
                    if (fish.staminaRatio() > 0.55f) priority = 50;
                    break;
            }

            if (priority > bestPriority) {
                bestPriority = priority;
                best = i;
            }
        }
        return best;
    }

    private boolean shouldAbort(FishState fish) {
        if (strategy.minRarity != null && fish.rarity.ordinal() < strategy.minRarity.ordinal()) {
            return true;
        }
        return strategy.minWeight > 0f && fish.weight < strategy.minWeight;
    }

    private static float clamp01(float v) {
        return v < 0f ? 0f : (v > 1f ? 1f : v);
    }

    /** Convenience for tooling: the skills a policy would want, in preference order. */
    public static boolean wantsSkill(SkillDef def, AutoStrategy strategy) {
        if (def.effect == SkillDef.Effect.BRACE || def.effect == SkillDef.Effect.MEND) {
            return strategy.riskTolerance < 0.7f;
        }
        return true;
    }
}
