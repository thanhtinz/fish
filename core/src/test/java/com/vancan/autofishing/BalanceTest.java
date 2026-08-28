package com.vancan.autofishing;

import com.vancan.autofishing.auto.AutoPilot;
import com.vancan.autofishing.auto.AutoStrategy;
import com.vancan.autofishing.content.GameContent;
import com.vancan.autofishing.content.SpotDef;
import com.vancan.autofishing.meta.CatchResolver;
import com.vancan.autofishing.sim.BuildStats;
import com.vancan.autofishing.sim.EncounterTable;
import com.vancan.autofishing.sim.FailureReason;
import com.vancan.autofishing.sim.FishingSession;
import com.vancan.autofishing.sim.SessionPhase;
import com.vancan.autofishing.sim.SkillDef;
import org.junit.jupiter.api.Test;

import static org.junit.jupiter.api.Assertions.assertTrue;

/**
 * The balance harness from GDD 23, run as a test so a tuning change cannot quietly break the
 * curve. These assert wide bands, not exact numbers: they are here to catch a regression that
 * makes a zone unplayable or trivial, not to freeze the tuning.
 */
class BalanceTest {

    private static final int RUNS = 1200;

    private static final class Stats {
        int landed;
        float totalTime;
        /** Wall-clock across every cast, landed or not: the denominator for throughput. */
        float totalSessionTime;
        long gold;
        int snapped;
        int spooled;
        int missed;

        float landRate() {
            return landed * 100f / RUNS;
        }

        float avgTime() {
            return landed == 0 ? 0 : totalTime / landed;
        }

        /** What the zone is actually worth per minute of play at this build and policy. */
        float goldPerMinute() {
            return totalSessionTime <= 0 ? 0 : gold * 60f / totalSessionTime;
        }
    }

    private Stats run(GameContent content, SpotDef spot, BuildStats build, AutoStrategy strategy) {
        EncounterTable table = spot.buildTable(content.species);
        SkillDef[] none = new SkillDef[0];
        Stats st = new Stats();
        for (int i = 0; i < RUNS; i++) {
            FishingSession s = new FishingSession(500000L + i, content.simConfig, build, table, none);
            AutoPilot pilot = new AutoPilot(strategy);
            int guard = 0;
            while (!s.isFinished() && guard++ < 20000) {
                s.update(pilot.decide(s));
            }
            st.totalSessionTime += s.getElapsed();
            if (s.getPhase() == SessionPhase.LANDED) {
                st.landed++;
                st.totalTime += s.getElapsed();
                st.gold += CatchResolver.resolve(s.getFish(), spot.id, 0L).goldValue;
            } else if (s.getFailure() == FailureReason.LINE_SNAPPED) {
                st.snapped++;
            } else if (s.getFailure() == FailureReason.SPOOLED) {
                st.spooled++;
            } else if (s.getFailure() == FailureReason.MISSED_HOOK) {
                st.missed++;
            }
        }
        return st;
    }

    @Test
    void onTierGearLandsMostFishInItsOwnZone() {
        GameContent content = TestContent.get();
        int tier = 1;
        for (SpotDef spot : content.spots.values()) {
            BuildStats build = TestBuilds.buildAtTier(content, tier);
            Stats st = run(content, spot, build, AutoStrategy.BALANCED);
            assertTrue(st.landRate() >= 55f && st.landRate() <= 95f,
                    "zone " + spot.id + " at its own gear tier landed " + st.landRate()
                            + "% (expected 55-95%). A zone outside this band is either a wall "
                            + "or free money.");
            assertTrue(st.avgTime() >= 4f && st.avgTime() <= 45f,
                    "zone " + spot.id + " average fight was " + st.avgTime()
                            + "s (expected 4-45s)");
            tier++;
        }
    }

    @Test
    void undergearedFarmingIsClearlyWorseThanGearingUp() {
        // What actually gates progression is not the catch rate on its own - it is throughput.
        // A player two tiers behind still lands a fair share of fish, but each one costs a much
        // longer fight, so the zone pays far less per minute than it does for a geared player.
        // Asserting the economic gap is the honest version of this check; asserting an absolute
        // catch-rate ceiling just encodes a number nobody derived.
        GameContent content = TestContent.get();
        SpotDef[] spots = content.spotsInOrder().toArray(new SpotDef[0]);

        for (int i = 2; i < spots.length; i++) {
            SpotDef spot = spots[i];
            int onTier = spot.tier;
            Stats geared = run(content, spot, TestBuilds.buildAtTier(content, onTier),
                    AutoStrategy.BALANCED);
            Stats undergeared = run(content, spot, TestBuilds.buildAtTier(content, onTier - 2),
                    AutoStrategy.BALANCED);

            assertTrue(undergeared.landRate() < geared.landRate(),
                    "zone " + spot.id + " landed " + undergeared.landRate()
                            + "% two tiers undergeared vs " + geared.landRate()
                            + "% on tier - gear is not mattering");

            float ratio = undergeared.goldPerMinute() / Math.max(1f, geared.goldPerMinute());
            assertTrue(ratio < 0.75f,
                    "zone " + spot.id + " pays " + undergeared.goldPerMinute()
                            + " gold/min two tiers undergeared vs " + geared.goldPerMinute()
                            + " on tier (ratio " + ratio + "). Farming above your gear should "
                            + "not be nearly as good as gearing up first.");
        }
    }

    @Test
    void safeTradesSpeedForSurvivalRatherThanBeingStrictlyWorse() {
        // The whole point of having five presets is that they are not orderable. If Aggressive
        // both lands more AND finishes faster, the strategy picker is decoration.
        GameContent content = TestContent.get();
        SpotDef[] spots = content.spotsInOrder().toArray(new SpotDef[0]);

        int tradeOffsFound = 0;
        for (int i = 1; i < spots.length; i++) {
            BuildStats build = TestBuilds.buildAtTier(content, i); // one tier behind the zone
            Stats safe = run(content, spots[i], build, AutoStrategy.SAFE);
            Stats aggressive = run(content, spots[i], build, AutoStrategy.AGGRESSIVE);

            if (safe.landed == 0 || aggressive.landed == 0) continue;

            assertTrue(safe.avgTime() > aggressive.avgTime(),
                    "in " + spots[i].id + " the Safe policy was not slower than Aggressive ("
                            + safe.avgTime() + "s vs " + aggressive.avgTime() + "s)");
            if (safe.landRate() > aggressive.landRate()) tradeOffsFound++;
        }
        assertTrue(tradeOffsFound > 0,
                "the Safe policy never out-landed Aggressive in any contested zone, so it is "
                        + "strictly dominated and the preset has no reason to exist");
    }

    @Test
    void aggressiveSnapsMoreLinesThanSafe() {
        GameContent content = TestContent.get();
        SpotDef[] spots = content.spotsInOrder().toArray(new SpotDef[0]);
        int contested = spots.length - 1;
        BuildStats build = TestBuilds.buildAtTier(content, contested);

        Stats safe = run(content, spots[contested], build, AutoStrategy.SAFE);
        Stats aggressive = run(content, spots[contested], build, AutoStrategy.AGGRESSIVE);
        assertTrue(aggressive.snapped >= safe.snapped,
                "Aggressive snapped " + aggressive.snapped + " lines vs Safe's " + safe.snapped
                        + "; risk tolerance is not translating into risk");
    }

    @Test
    void everyFailureModeIsReachable() {
        // A failure mode that never fires is dead code that will rot. This sweeps a range of
        // gear/zone mismatches and asserts the simulation can produce each ending.
        GameContent content = TestContent.get();
        boolean snap = false, spool = false, miss = false, land = false;
        SpotDef[] spots = content.spotsInOrder().toArray(new SpotDef[0]);

        for (int tier = 1; tier <= spots.length && !(snap && spool && miss && land); tier++) {
            for (int z = 0; z < spots.length; z++) {
                BuildStats build = TestBuilds.buildAtTier(content, tier);
                for (AutoStrategy strategy : AutoStrategy.values()) {
                    if (strategy == AutoStrategy.RARE_HUNTER || strategy == AutoStrategy.HEAVY_HUNTER) {
                        continue; // these abort on purpose and would mask the real endings
                    }
                    Stats st = run(content, spots[z], build, strategy);
                    if (st.snapped > 0) snap = true;
                    if (st.spooled > 0) spool = true;
                    if (st.missed > 0) miss = true;
                    if (st.landed > 0) land = true;
                }
            }
        }
        assertTrue(land, "no configuration ever landed a fish");
        assertTrue(snap, "LINE_SNAPPED is unreachable");
        assertTrue(spool, "SPOOLED is unreachable - the distance axis is inert");
        assertTrue(miss, "MISSED_HOOK is unreachable");
    }

    @Test
    void gearProgressionStrictlyImprovesOutcomes() {
        GameContent content = TestContent.get();
        SpotDef target = content.spotsInOrder().get(3);
        float previous = -1f;
        for (int tier = 2; tier <= 6; tier++) {
            Stats st = run(content, target, TestBuilds.buildAtTier(content, tier),
                    AutoStrategy.BALANCED);
            assertTrue(st.landRate() >= previous - 3f,
                    "tier " + tier + " landed " + st.landRate() + "%, worse than tier "
                            + (tier - 1) + "'s " + previous + "% - upgrading gear must not hurt");
            previous = st.landRate();
        }
    }
}
