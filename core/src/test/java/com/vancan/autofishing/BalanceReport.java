package com.vancan.autofishing;

import com.vancan.autofishing.auto.AutoPilot;
import com.vancan.autofishing.auto.AutoStrategy;
import com.vancan.autofishing.content.GameContent;
import com.vancan.autofishing.content.SpotDef;
import com.vancan.autofishing.sim.BuildStats;
import com.vancan.autofishing.sim.EncounterTable;
import com.vancan.autofishing.sim.FishingSession;
import com.vancan.autofishing.sim.SessionPhase;
import com.vancan.autofishing.sim.SkillDef;

/**
 * Designer-facing balance sweep (GDD 23). Run with {@code ./gradlew balanceReport}.
 *
 * <p>{@link BalanceTest} asserts wide guard-rails so CI fails on a real regression; this prints the
 * full grid so whoever is tuning can see <em>where</em> the curve moved and by how much. It is a
 * main() rather than a test because its value is the table, not a pass/fail.
 */
public final class BalanceReport {

    private static final int RUNS = 2000;

    public static void main(String[] args) {
        GameContent content = TestContent.get();
        SpotDef[] spots = content.spotsInOrder().toArray(new SpotDef[0]);

        System.out.println("Van Can Auto Fishing - balance sweep (" + RUNS + " casts per cell)");
        System.out.println("gold/min assumes back-to-back Auto casting with no travel time.\n");

        for (int z = 0; z < spots.length; z++) {
            for (int gearTier = Math.max(1, z); gearTier <= Math.min(6, z + 2); gearTier++) {
                BuildStats build = TestBuilds.buildAtTier(content, gearTier);
                System.out.println(spots[z].name + "  (zone tier " + spots[z].tier
                        + ", gear tier " + gearTier + ")");
                for (AutoStrategy strategy : AutoStrategy.values()) {
                    report(content, spots[z], build, strategy);
                }
                System.out.println();
            }
        }
    }

    private static void report(GameContent content, SpotDef spot, BuildStats build,
                               AutoStrategy strategy) {
        EncounterTable table = spot.buildTable(content.species);
        SkillDef[] skills = new SkillDef[0];
        int landed = 0;
        float totalTime = 0f, totalSessionTime = 0f;
        long gold = 0;
        int[] failures = new int[com.vancan.autofishing.sim.FailureReason.values().length];

        for (int i = 0; i < RUNS; i++) {
            FishingSession s = new FishingSession(900000L + i, content.simConfig, build, table, skills);
            AutoPilot pilot = new AutoPilot(strategy);
            int guard = 0;
            while (!s.isFinished() && guard++ < 20000) {
                s.update(pilot.decide(s));
            }
            totalSessionTime += s.getElapsed();
            if (s.getPhase() == SessionPhase.LANDED) {
                landed++;
                totalTime += s.getElapsed();
                gold += com.vancan.autofishing.meta.CatchResolver
                        .resolve(s.getFish(), spot.id, 0L).goldValue;
            } else {
                failures[s.getFailure().ordinal()]++;
            }
        }

        float goldPerMinute = totalSessionTime <= 0 ? 0 : gold * 60f / totalSessionTime;
        StringBuilder fail = new StringBuilder();
        for (com.vancan.autofishing.sim.FailureReason r
                : com.vancan.autofishing.sim.FailureReason.values()) {
            if (failures[r.ordinal()] > 0) {
                fail.append(' ').append(r.name().toLowerCase()).append('=')
                    .append(failures[r.ordinal()] * 100 / RUNS).append('%');
            }
        }

        System.out.printf("   %-12s land=%5.1f%%  fight=%5.1fs  gold/min=%7.0f %s%n",
                strategy.displayName, landed * 100f / RUNS,
                landed == 0 ? 0f : totalTime / landed, goldPerMinute, fail);
    }
}
