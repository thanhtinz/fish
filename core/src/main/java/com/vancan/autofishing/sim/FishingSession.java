package com.vancan.autofishing.sim;

/**
 * The fishing simulation (GDD 5). One instance == one cast.
 *
 * <p>Three properties are load-bearing and everything else is arranged around them:
 *
 * <ul>
 *   <li><b>Deterministic.</b> Same seed + same {@link BuildStats} + same action sequence == same
 *       result, bit for bit, on every platform. That is what makes server-side re-simulation a
 *       viable anti-cheat (GDD 19) instead of a heuristic.
 *   <li><b>Fixed step.</b> {@link #update} always advances by {@link SimConfig#tickSeconds}. The
 *       renderer accumulates real frame time and calls it as often as needed, so a 30fps phone and
 *       a 144fps desktop fight the same fish.
 *   <li><b>Headless.</b> No libGDX types are referenced here, so the same class runs in the
 *       balance harness, in unit tests, and (unchanged) inside a JVM game server.
 * </ul>
 */
public final class FishingSession {

    private final SimConfig cfg;
    private final BuildStats build;
    private final EncounterTable table;
    private final Rng rng;
    private final long seed;
    private final SkillRuntime[] skills;

    private SessionPhase phase = SessionPhase.SEARCHING;
    private FailureReason failure = FailureReason.NONE;
    private FishState fish;

    private float tension;
    private float lineIntegrity = 1f;
    private float distance;
    private float elapsed;
    private float phaseElapsed;

    /** Seconds the line has been effectively slack; long slack lets the hook fall out. */
    private float slackTimer;

    // --- Telemetry (GDD 14 "Perfect Auto", GDD 24) ---------------------------------------
    private float tensionWaste;
    private float peakTension;
    private int critCount;
    private int tickCount;

    public FishingSession(long seed, SimConfig cfg, BuildStats build, EncounterTable table,
                          SkillDef[] equippedSkills) {
        this.seed = seed;
        this.cfg = cfg;
        this.build = build;
        this.table = table;
        this.rng = new Rng(seed);
        this.skills = new SkillRuntime[equippedSkills == null ? 0 : equippedSkills.length];
        for (int i = 0; i < this.skills.length; i++) {
            this.skills[i] = new SkillRuntime(equippedSkills[i]);
        }
        this.distance = 0f;
    }

    /** Advances exactly one fixed tick. Returns the phase after the tick. */
    public SessionPhase update(FishingAction action) {
        if (isFinished()) return phase;

        float dt = cfg.tickSeconds;
        elapsed += dt;
        phaseElapsed += dt;
        tickCount++;

        for (int i = 0; i < skills.length; i++) {
            skills[i].tick(dt);
        }

        switch (phase) {
            case SEARCHING:
                updateSearching(dt);
                break;
            case BITE:
                updateBite(action, dt);
                break;
            case FIGHT:
                updateFight(action, dt);
                break;
            default:
                break;
        }
        return phase;
    }

    // -----------------------------------------------------------------------------------
    // SEARCHING
    // -----------------------------------------------------------------------------------

    private float biteAt = -1f;

    private void updateSearching(float dt) {
        if (biteAt < 0f) {
            // Bait attraction and float detection shorten the wait; the exponential draw keeps
            // the wait feeling organic rather than metronomic.
            float rate = Math.max(0.05f, build.attraction * build.biteDetection);
            float mean = cfg.baseBiteIntervalSeconds / rate;
            biteAt = (float) (-Math.log(1.0 - rng.nextDouble()) * mean);
        }

        if (phaseElapsed >= cfg.searchTimeoutSeconds) {
            fail(FailureReason.NO_BITE);
            return;
        }

        if (phaseElapsed >= biteAt) {
            fish = FishState.of(table.roll(rng, build), rng);
            fish.weight *= build.weightModifier;
            enter(SessionPhase.BITE);
        }
    }

    // -----------------------------------------------------------------------------------
    // BITE — the hook-set window
    // -----------------------------------------------------------------------------------

    private static final float HOOK_INTENT_THRESHOLD = 0.25f;

    private void updateBite(FishingAction action, float dt) {
        boolean striking = action != null && action.pullLevel >= HOOK_INTENT_THRESHOLD;

        if (striking) {
            // Striking late in the window is worse: the fish has had time to spit the bait.
            float windowQuality = 1f - (phaseElapsed / cfg.biteWindowSeconds) * 0.35f;
            float chance = build.hookRate * windowQuality / Math.max(0.2f, fish.species.hookDifficulty);
            chance += build.critHook;
            if (rng.chance(Math.min(0.98f, chance))) {
                enter(SessionPhase.FIGHT);
                tension = fish.currentForce(cfg) * 0.5f;
                distance = build.lineLength * cfg.castDistanceRatio;
            } else {
                fail(FailureReason.MISSED_HOOK);
            }
            return;
        }

        if (phaseElapsed >= cfg.biteWindowSeconds) {
            fail(FailureReason.MISSED_HOOK);
        }
    }

    // -----------------------------------------------------------------------------------
    // FIGHT
    // -----------------------------------------------------------------------------------

    private void updateFight(FishingAction action, float dt) {
        if (action != null && action.retreat) {
            fail(FailureReason.RETREAT);
            return;
        }

        float pull = action == null ? 0.6f : clamp01(action.pullLevel);

        if (action != null && action.skillIndex >= 0 && action.skillIndex < skills.length) {
            SkillRuntime s = skills[action.skillIndex];
            if (s.isReady()) s.trigger();
        }

        // --- Resolve active skill effects into plain multipliers -------------------------
        float damageMul = build.damageBonus;
        float braceForce = build.safetyMitigation;
        float reelMul = 1f;
        float exhaustDrain = 0f;
        for (int i = 0; i < skills.length; i++) {
            SkillRuntime s = skills[i];
            if (!s.isActive()) continue;
            switch (s.def.effect) {
                case BURST:   damageMul *= s.def.magnitude; break;
                case BRACE:   braceForce += s.def.magnitude; break;
                case WINCH:   reelMul *= s.def.magnitude; break;
                case EXHAUST: exhaustDrain += s.def.magnitude; break;
                case MEND:
                    lineIntegrity = Math.min(1f, lineIntegrity + s.def.magnitude * dt);
                    break;
            }
        }

        // A phase change lands as an instant shock, not a smooth ramp: this is the moment a
        // fight is actually won or lost, and it is what the Auto policy's phase rules exist for.
        FishPhase before = fish.phase;
        float forceBefore = fish.currentForce(cfg);
        fish.tickPhase(dt, rng);
        if (fish.phase != before) {
            float jump = fish.currentForce(cfg) - forceBefore;
            if (jump > 0f) tension += jump * cfg.phaseShockFactor;
        }

        // --- Forces (GDD 5.3) -----------------------------------------------------------
        float effectivePull = (build.rodPower + build.reelPower + build.teamPull) * pull;
        float fishForce = fish.currentForce(cfg);

        float tensionTarget = fishForce
                + effectivePull * cfg.pullTensionCoefficient
                - braceForce
                - build.elasticity * cfg.elasticityRelief;
        if (tensionTarget < 0f) tensionTarget = 0f;

        // Control makes the line track its target more smoothly, which is what "a good rod feels
        // forgiving" actually means numerically.
        float response = cfg.lineResponse / Math.max(0.4f, build.control);
        tension += (tensionTarget - tension) * Math.min(1f, response * dt);
        if (tension < 0f) tension = 0f;
        if (tension > peakTension) peakTension = tension;

        // --- Line wear ------------------------------------------------------------------
        float breaking = build.breakingTension();
        float safe = build.safeTension(cfg);
        if (tension > safe) {
            float over = tension - safe;
            tensionWaste += over * dt;
            lineIntegrity -= (over / breaking) * cfg.wearRate * dt;
        } else {
            lineIntegrity = Math.min(1f, lineIntegrity + build.recovery * 0.004f * dt);
        }

        if (tension >= breaking || lineIntegrity <= 0f) {
            lineIntegrity = Math.max(0f, lineIntegrity);
            fail(FailureReason.LINE_SNAPPED);
            return;
        }

        // --- Damage and fatigue ---------------------------------------------------------
        boolean crit = rng.chance(build.critChance);
        if (crit) critCount++;
        float damage = effectivePull * cfg.damageCoefficient * damageMul
                * (crit ? cfg.critMultiplier : 1f);
        fish.hp -= damage * dt;
        fish.stamina -= (effectivePull * cfg.staminaDamageCoefficient + exhaustDrain) * dt;

        // Giving slack lets the fish recover — this is the cost that stops "hold at zero pull"
        // from being a free strategy, and it is why the Safe policy still has to commit.
        if (pull < 0.15f) {
            slackTimer += dt;
            fish.stamina = Math.min(fish.maxStamina,
                    fish.stamina + fish.maxStamina * cfg.fishStaminaRecovery * dt);
            if (slackTimer > 2.5f && rng.chance(fish.species.escapeRate * dt)) {
                fail(FailureReason.FISH_ESCAPED);
                return;
            }
        } else {
            slackTimer = 0f;
        }

        if (fish.stamina < 0f) fish.stamina = 0f;
        if (fish.hp < 0f) fish.hp = 0f;

        // --- Distance -------------------------------------------------------------------
        boolean exhausted = fish.stamina <= 0f || fish.hp <= 0f;
        float drive = exhausted ? 0f : fish.currentDrive() * (1f - clamp01(build.distanceControl));
        float reelSpeed = build.pullSpeed * pull * cfg.reelSpeedCoefficient * reelMul;
        distance += (drive - reelSpeed) * dt;
        if (distance < 0f) distance = 0f;

        if (distance >= build.lineLength) {
            distance = build.lineLength;
            fail(FailureReason.SPOOLED);
            return;
        }

        // --- Resolution -----------------------------------------------------------------
        if (exhausted && distance <= cfg.landingThreshold) {
            phase = SessionPhase.LANDED;
            return;
        }

        if (elapsed >= cfg.fightTimeoutSeconds) {
            fail(FailureReason.TIMEOUT);
        }
    }

    // -----------------------------------------------------------------------------------

    private void enter(SessionPhase next) {
        phase = next;
        phaseElapsed = 0f;
    }

    private void fail(FailureReason reason) {
        failure = reason;
        phase = SessionPhase.FAILED;
    }

    private static float clamp01(float v) {
        return v < 0f ? 0f : (v > 1f ? 1f : v);
    }

    // --- Observation surface -------------------------------------------------------------

    public boolean isFinished() {
        return phase == SessionPhase.LANDED || phase == SessionPhase.FAILED;
    }

    public SessionPhase getPhase() {
        return phase;
    }

    public FailureReason getFailure() {
        return failure;
    }

    public FishState getFish() {
        return fish;
    }

    public long getSeed() {
        return seed;
    }

    public float getTension() {
        return tension;
    }

    /** Tension as a 0..1 fraction of the breaking point — what the HUD gauge shows. */
    public float getTensionRatio() {
        float breaking = build.breakingTension();
        return breaking <= 0 ? 0 : Math.min(1f, tension / breaking);
    }

    public float getSafeTensionRatio() {
        return cfg.safeTensionRatio;
    }

    public float getLineIntegrity() {
        return lineIntegrity;
    }

    public float getDistance() {
        return distance;
    }

    public float getDistanceRatio() {
        return build.lineLength <= 0 ? 0 : Math.min(1f, distance / build.lineLength);
    }

    public float getElapsed() {
        return elapsed;
    }

    public float getPhaseElapsed() {
        return phaseElapsed;
    }

    public float getBiteWindowRatio() {
        return Math.min(1f, phaseElapsed / cfg.biteWindowSeconds);
    }

    public SkillRuntime[] getSkills() {
        return skills;
    }

    public BuildStats getBuild() {
        return build;
    }

    public SimConfig getConfig() {
        return cfg;
    }

    public float getTensionWaste() {
        return tensionWaste;
    }

    public float getPeakTension() {
        return peakTension;
    }

    public int getCritCount() {
        return critCount;
    }

    public int getTickCount() {
        return tickCount;
    }

    /**
     * Efficiency score for the "Perfect Auto" competitive mode (GDD 14): landing fast while
     * wasting as little tension headroom as possible.
     */
    public float efficiencyScore() {
        if (phase != SessionPhase.LANDED) return 0f;
        float wastePenalty = 1f / (1f + tensionWaste * 0.02f);
        float speedScore = 1f / (1f + elapsed * 0.05f);
        return 1000f * wastePenalty * speedScore;
    }
}
