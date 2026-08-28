package com.vancan.autofishing;

import com.vancan.autofishing.auto.AutoPilot;
import com.vancan.autofishing.auto.AutoStrategy;
import com.vancan.autofishing.sim.BuildStats;
import com.vancan.autofishing.sim.EncounterTable;
import com.vancan.autofishing.sim.FishingAction;
import com.vancan.autofishing.sim.FishingSession;
import com.vancan.autofishing.sim.Rng;
import com.vancan.autofishing.sim.SessionPhase;
import com.vancan.autofishing.sim.SimConfig;
import com.vancan.autofishing.sim.SkillDef;
import com.vancan.autofishing.sim.SpeciesDef;
import org.junit.jupiter.api.Test;

import static org.junit.jupiter.api.Assertions.assertEquals;
import static org.junit.jupiter.api.Assertions.assertTrue;

/**
 * Determinism is the property the anti-cheat design in GDD 19 rests on: the server has to be able
 * to replay a client's session from its seed and action log and land on the same result.
 */
class SimulationTest {

    private static final int GUARD = 20000;

    private FishingSession session(long seed, SpeciesDef species, BuildStats build) {
        EncounterTable table = new EncounterTable();
        table.add(species, 1f);
        return new FishingSession(seed, TestContent.get().simConfig, build, table, new SkillDef[0]);
    }

    private SpeciesDef anySpecies() {
        return TestContent.get().species.get("ca_chep");
    }

    private BuildStats midBuild() {
        BuildStats b = new BuildStats();
        b.rodPower = 16; b.reelPower = 9; b.lineStrength = 85; b.maxDrag = 85;
        b.pullSpeed = 3.6f; b.lineLength = 32; b.hookRate = 0.90f;
        return b;
    }

    @Test
    void sameSeedProducesIdenticalOutcome() {
        for (long seed = 1; seed <= 40; seed++) {
            Outcome a = play(seed);
            Outcome b = play(seed);
            assertEquals(a.phase, b.phase, "phase diverged for seed " + seed);
            assertEquals(a.failure, b.failure, "failure diverged for seed " + seed);
            assertEquals(a.ticks, b.ticks, "tick count diverged for seed " + seed);
            assertEquals(a.weight, b.weight, 0f, "weight diverged for seed " + seed);
            assertEquals(a.tension, b.tension, 0f, "tension diverged for seed " + seed);
        }
    }

    @Test
    void differentSeedsProduceDifferentFights() {
        // If seeds collapsed onto one outcome the RNG would be wired up wrong and replay
        // verification would pass for any forged seed.
        int distinctTickCounts = 0;
        int previous = -1;
        for (long seed = 1; seed <= 30; seed++) {
            int ticks = play(seed).ticks;
            if (ticks != previous) distinctTickCounts++;
            previous = ticks;
        }
        assertTrue(distinctTickCounts > 10,
                "seeds barely vary the fight (" + distinctTickCounts + " changes in 30)");
    }

    private static final class Outcome {
        SessionPhase phase;
        Object failure;
        int ticks;
        float weight;
        float tension;
    }

    private Outcome play(long seed) {
        FishingSession s = session(seed, anySpecies(), midBuild());
        AutoPilot pilot = new AutoPilot(AutoStrategy.BALANCED);
        int guard = 0;
        while (!s.isFinished() && guard++ < GUARD) {
            s.update(pilot.decide(s));
        }
        Outcome o = new Outcome();
        o.phase = s.getPhase();
        o.failure = s.getFailure();
        o.ticks = s.getTickCount();
        o.weight = s.getFish() == null ? 0f : s.getFish().weight;
        o.tension = s.getTension();
        return o;
    }

    @Test
    void everySessionTerminates() {
        // A session that never ends would hang the Auto loop and, worse, the offline settlement.
        for (long seed = 1; seed <= 300; seed++) {
            FishingSession s = session(seed, anySpecies(), midBuild());
            AutoPilot pilot = new AutoPilot(AutoStrategy.SAFE);
            int guard = 0;
            while (!s.isFinished() && guard++ < GUARD) {
                s.update(pilot.decide(s));
            }
            assertTrue(s.isFinished(), "session did not terminate for seed " + seed);
        }
    }

    @Test
    void tensionAndIntegrityStayInBounds() {
        for (long seed = 1; seed <= 200; seed++) {
            FishingSession s = session(seed, anySpecies(), midBuild());
            AutoPilot pilot = new AutoPilot(AutoStrategy.AGGRESSIVE);
            int guard = 0;
            while (!s.isFinished() && guard++ < GUARD) {
                s.update(pilot.decide(s));
                assertTrue(s.getTension() >= 0f, "negative tension");
                assertTrue(s.getLineIntegrity() >= 0f && s.getLineIntegrity() <= 1.0001f,
                        "line integrity out of range: " + s.getLineIntegrity());
                assertTrue(s.getDistance() >= 0f, "negative distance");
                assertTrue(s.getTensionRatio() <= 1.0001f, "tension ratio over 1");
            }
        }
    }

    @Test
    void slackLetsTheFishRecoverSoStallingIsNotFree() {
        // The "hold at zero pull" exploit: if giving slack were free, every policy would collapse
        // into waiting the fish out and gear would stop mattering.
        SpeciesDef species = anySpecies();
        FishingSession s = session(99, species, midBuild());
        FishingAction strike = new FishingAction(1f);
        int guard = 0;
        while (s.getPhase() != SessionPhase.FIGHT && !s.isFinished() && guard++ < GUARD) {
            s.update(strike);
        }
        assertEquals(SessionPhase.FIGHT, s.getPhase(), "never reached the fight");

        FishingAction pull = new FishingAction(0.9f);
        for (int i = 0; i < 60 && !s.isFinished(); i++) s.update(pull);
        float drained = s.getFish().stamina;

        FishingAction slack = new FishingAction(0f);
        for (int i = 0; i < 60 && !s.isFinished(); i++) s.update(slack);

        assertTrue(s.isFinished() || s.getFish().stamina > drained,
                "stamina did not recover on slack line; stalling would be a free strategy");
    }

    @Test
    void rngIsStableAcrossInstances() {
        Rng a = new Rng(12345);
        Rng b = new Rng(12345);
        for (int i = 0; i < 1000; i++) {
            assertEquals(a.nextDouble(), b.nextDouble(), 0.0);
        }
        Rng r = new Rng(7);
        for (int i = 0; i < 10000; i++) {
            double v = r.nextDouble();
            assertTrue(v >= 0.0 && v < 1.0, "nextDouble out of range: " + v);
        }
    }

    @Test
    void configuredTickIsRespected() {
        SimConfig cfg = TestContent.get().simConfig;
        FishingSession s = session(5, anySpecies(), midBuild());
        s.update(new FishingAction(0.3f));
        assertEquals(cfg.tickSeconds, s.getElapsed(), 1e-6f);
    }
}
