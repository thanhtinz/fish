package com.vancan.autofishing.meta;

import com.vancan.autofishing.auto.AutoPilot;
import com.vancan.autofishing.auto.AutoStrategy;
import com.vancan.autofishing.content.GameContent;
import com.vancan.autofishing.content.SpotDef;
import com.vancan.autofishing.sim.BuildStats;
import com.vancan.autofishing.sim.EncounterTable;
import com.vancan.autofishing.sim.FailureReason;
import com.vancan.autofishing.sim.FishingAction;
import com.vancan.autofishing.sim.FishingSession;
import com.vancan.autofishing.sim.SessionPhase;
import com.vancan.autofishing.sim.SkillDef;

/**
 * Drives the cast-fight-settle loop that sits above a single {@link FishingSession}.
 *
 * <p>Kept out of the screen so the loop can be unit-tested and so the identical code can run
 * server-side. The screen only reads state and draws it.
 *
 * <p>Time is accumulated and consumed in whole fixed ticks. Feeding the simulation a raw frame
 * delta would make the fight resolve differently on a 60Hz phone and a 144Hz desktop, and would
 * break the replay guarantee the anti-cheat design depends on.
 */
public final class FishingController {

    /** Seconds of result screen before Auto casts again. */
    private static final float RECAST_DELAY = 1.4f;

    private final GameContent content;
    private final PlayerState player;
    private final AutoPilot pilot;

    private FishingSession session;
    private BuildStats build;
    private EncounterTable table;
    private SpotDef spot;
    private SkillDef[] skills;

    private float accumulator;
    private float resultTimer;
    private long castSeed;

    /** The most recent finished cast, kept for the result panel. */
    private FishRecord lastCatch;
    private FailureReason lastFailure = FailureReason.NONE;
    private String lastFishName;
    private float lastFishWeight;
    private int levelsGainedOnLastCatch;

    /** Manual pull level, used when Auto is off. */
    public float manualPull = 0.6f;
    /** Pull level applied on the most recent tick. The renderer poses the angler from it. */
    private float lastPull;
    /** Skill the player tapped this frame; consumed by the next tick. */
    private int requestedSkill = -1;

    public FishingController(GameContent content, PlayerState player) {
        this.content = content;
        this.player = player;
        this.pilot = new AutoPilot(player.autoStrategy);
        rebuild();
    }

    /** Re-resolves gear, team and spot. Call after any inventory or spot change. */
    public void rebuild() {
        build = LoadoutResolver.resolve(player, content);
        skills = LoadoutResolver.resolveSkills(player, content);
        spot = content.spots.get(player.currentSpotId);
        table = spot == null ? null : spot.buildTable(content.species);
        pilot.setStrategy(player.autoStrategy);
    }

    public void cast() {
        if (table == null) return;
        castSeed = nextSeed();
        session = new FishingSession(castSeed, content.simConfig, build, table, skills);
        accumulator = 0f;
        resultTimer = 0f;
    }

    /**
     * Seeds come from the wall clock plus a counter today. In the server-authoritative build this
     * is where the client would instead ask the game service to open a session and hand back the
     * seed, so the client never picks the fish it is about to catch (GDD 19).
     */
    private long seedCounter;

    private long nextSeed() {
        return com.vancan.autofishing.sim.Rng.derive(
                System.currentTimeMillis() ^ (++seedCounter * 0x9E3779B9L),
                player.playerId).nextLong();
    }

    public void update(float delta, long now) {
        if (spot == null) return;

        if (session == null) {
            if (player.autoEnabled) cast();
            return;
        }

        if (session.isFinished()) {
            resultTimer += delta;
            if (player.autoEnabled && resultTimer >= RECAST_DELAY) cast();
            return;
        }

        pilot.setStrategy(player.autoStrategy);
        float tick = content.simConfig.tickSeconds;
        accumulator += delta;

        // Bound the catch-up work: after a long stall, resolve at most this many ticks per frame
        // so the app stays responsive instead of freezing while it replays lost time.
        int budget = 240;
        while (accumulator >= tick && !session.isFinished() && budget-- > 0) {
            accumulator -= tick;
            session.update(nextAction());
        }
        if (budget <= 0) accumulator = 0f;

        if (session.isFinished()) {
            settle(now);
        }
    }

    private final FishingAction manualAction = new FishingAction();

    private FishingAction nextAction() {
        if (player.autoEnabled) {
            FishingAction a = pilot.decide(session);
            if (requestedSkill >= 0) {
                a.skillIndex = requestedSkill;
                requestedSkill = -1;
            }
            lastPull = a.pullLevel;
            return a;
        }
        // Manual: striking at a bite is automatic, because a bite window is 1.6s and asking the
        // player to also find a separate "strike" button in that time is not one-handed play.
        float pull = session.getPhase() == SessionPhase.BITE ? 1f : manualPull;
        lastPull = pull;
        manualAction.set(pull, requestedSkill, false);
        requestedSkill = -1;
        return manualAction;
    }

    private void settle(long now) {
        resultTimer = 0f;
        if (session.getPhase() == SessionPhase.LANDED) {
            FishRecord record = CatchResolver.resolve(session.getFish(), spot.id, now);
            player.recordCatch(record);
            player.applyCurrency(Currency.GOLD, record.goldValue,
                    "catch:" + record.speciesId, "catch:" + castSeed);
            levelsGainedOnLastCatch = player.addXp(record.xpValue);
            lastCatch = record;
            lastFailure = FailureReason.NONE;
            lastFishName = session.getFish().species.name;
            lastFishWeight = session.getFish().weight;
        } else {
            lastCatch = null;
            levelsGainedOnLastCatch = 0;
            lastFailure = session.getFailure();
            lastFishName = session.getFish() == null ? null : session.getFish().species.name;
            lastFishWeight = session.getFish() == null ? 0f : session.getFish().weight;
        }
    }

    public void requestSkill(int index) {
        requestedSkill = index;
    }

    public void setStrategy(AutoStrategy strategy) {
        player.autoStrategy = strategy;
        pilot.setStrategy(strategy);
    }

    public void toggleAuto() {
        player.autoEnabled = !player.autoEnabled;
        if (player.autoEnabled && session == null) cast();
    }

    public FishingSession session() {
        return session;
    }

    public SpotDef spot() {
        return spot;
    }

    public BuildStats build() {
        return build;
    }

    public SkillDef[] skills() {
        return skills;
    }

    public FishRecord lastCatch() {
        return lastCatch;
    }

    public FailureReason lastFailure() {
        return lastFailure;
    }

    public String lastFishName() {
        return lastFishName;
    }

    public float lastFishWeight() {
        return lastFishWeight;
    }

    public int levelsGainedOnLastCatch() {
        return levelsGainedOnLastCatch;
    }

    /** 0..1 pull applied on the last tick, for posing the angler and bending the rod. */
    public float lastPull() {
        return lastPull;
    }

    public boolean isIdle() {
        return session == null || session.isFinished();
    }
}
