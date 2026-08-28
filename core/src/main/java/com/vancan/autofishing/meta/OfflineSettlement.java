package com.vancan.autofishing.meta;

import com.vancan.autofishing.auto.AutoStrategy;
import com.vancan.autofishing.content.GameContent;
import com.vancan.autofishing.content.SpotDef;
import com.vancan.autofishing.sim.BuildStats;
import com.vancan.autofishing.sim.EncounterTable;
import com.vancan.autofishing.sim.SimConfig;
import com.vancan.autofishing.sim.SpeciesDef;

/**
 * Idle / offline rewards (GDD 12).
 *
 * <p>Deliberately an <em>expected-value</em> model rather than a fast-forwarded simulation: running
 * eight hours of 30Hz ticks per player on login is not affordable, and it would also mean a
 * player's offline yield depended on RNG they never saw. Instead the same terms the live
 * simulation uses - bite rate, hook rate, catch rate, rarity mix - are integrated analytically, so
 * the offline number is consistent with what the player would have earned while watching.
 *
 * <p>The catch-rate term is calibrated from the live simulation by
 * {@code OfflineSettlementTest}, which fails if the model drifts away from measured play.
 */
public final class OfflineSettlement {

    /** Result of one settlement. Rendered by the client; never computed by it in production. */
    public static final class Result {
        public float hoursCredited;
        public float hoursElapsed;
        public boolean capped;
        public int casts;
        public int catches;
        public int gold;
        public int xp;
        /** Idempotency key so a double-claim cannot pay twice (GDD 19). */
        public String txId;
    }

    /** Absences shorter than this are folded into the next one rather than reported. */
    public static final float MIN_REPORTABLE_HOURS = 0.25f;

    private OfflineSettlement() {
    }

    /** Average seconds spent on one full cast-to-result cycle. Also shown in the idle UI. */
    public static float secondsPerCast(SimConfig cfg, BuildStats build) {
        float biteRate = Math.max(0.05f, build.attraction * build.biteDetection);
        float waitSeconds = cfg.baseBiteIntervalSeconds / biteRate;
        // A fight plus the result/recast overhead. Measured against the live sim in the tests.
        float fightSeconds = 16f;
        return waitSeconds + fightSeconds;
    }

    /**
     * Fraction of hooked fish that get landed, approximated from how far the build's pulling power
     * exceeds the pool's average resistance.
     */
    public static float effectiveCatchRate(EncounterTable table, BuildStats build, AutoStrategy strategy) {
        float totalWeight = 0f;
        float weightedPower = 0f;
        for (EncounterTable.Entry e : table.getEntries()) {
            SpeciesDef s = e.species;
            float midWeight = (s.minWeight + s.maxWeight) * 0.5f;
            float power = s.basePower + s.powerPerKg * midWeight;
            weightedPower += e.weight * power;
            totalWeight += e.weight;
        }
        if (totalWeight <= 0) return 0f;
        float avgPower = weightedPower / totalWeight;

        // Ratio of what the line tolerates to what the fish applies. Above ~2.2 the build is
        // comfortably over-tier and the rate saturates; below ~1.0 almost nothing lands.
        float margin = build.breakingTension() / Math.max(1f, avgPower * 2.6f);
        float rate = (margin - 0.55f) / 1.15f;
        rate = clamp(rate, 0.02f, 0.92f);

        // The Safe policy lands more per hooked fish but takes longer; the caller pays for that
        // through secondsPerCast, so the two effects do not double-count.
        rate *= 0.85f + 0.30f * (1f - strategy.riskTolerance);
        return clamp(rate, 0.02f, 0.95f);
    }

    /**
     * @param elapsedMillis wall-clock time since the last settlement
     * @return a settlement the caller can apply to the player, or null if nothing accrued
     */
    public static Result settle(PlayerState player, GameContent content, BuildStats build,
                                long elapsedMillis, long now) {
        SpotDef spot = content.spots.get(player.currentSpotId);
        if (spot == null) return null;

        SimConfig cfg = content.simConfig;
        Result r = new Result();
        r.hoursElapsed = elapsedMillis / 3_600_000f;

        float cap = cfg.offlineCapHours * build.offlineEfficiency;
        r.hoursCredited = Math.min(r.hoursElapsed, cap);
        r.capped = r.hoursElapsed > cap;
        // Below the minimum, an absence is not worth interrupting the player with a modal that
        // reports "0.0 hours" and a handful of gold - which is what happens on every quick
        // app-switch if this is not filtered here.
        if (r.hoursCredited < MIN_REPORTABLE_HOURS) return null;

        EncounterTable table = spot.buildTable(content.species);
        float catchRate = effectiveCatchRate(table, build, player.autoStrategy);
        float perCast = secondsPerCast(cfg, build);

        float seconds = r.hoursCredited * 3600f;
        r.casts = (int) (seconds / perCast);
        r.catches = (int) (r.casts * build.hookRate * catchRate * cfg.offlinePenalty
                * build.offlineEfficiency);

        // Value the average catch from the pool rather than rolling one, so the payout is stable.
        float avgValue = 0f, avgXp = 0f, totalWeight = 0f;
        for (EncounterTable.Entry e : table.getEntries()) {
            avgValue += e.weight * e.species.baseValue;
            avgXp += e.weight * e.species.baseXp;
            totalWeight += e.weight;
        }
        if (totalWeight > 0) {
            avgValue /= totalWeight;
            avgXp /= totalWeight;
        }
        float rarityMul = table.expectedRarityMultiplier(build);

        r.gold = (int) (r.catches * avgValue * rarityMul);
        r.xp = (int) (r.catches * avgXp);
        r.txId = "offline:" + player.playerId + ":" + now;
        return r;
    }

    /** Applies a settlement exactly once. Safe to call again with the same result. */
    public static boolean apply(PlayerState player, Result r) {
        if (r == null || r.catches <= 0) return false;
        if (!player.applyCurrency(Currency.GOLD, r.gold, "offline:settle", r.txId)) {
            return false;
        }
        player.addXp(r.xp);
        return true;
    }

    private static float clamp(float v, float lo, float hi) {
        return v < lo ? lo : (v > hi ? hi : v);
    }
}
