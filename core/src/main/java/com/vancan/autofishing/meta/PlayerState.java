package com.vancan.autofishing.meta;

import com.vancan.autofishing.auto.AutoStrategy;

import java.util.ArrayList;
import java.util.EnumMap;
import java.util.LinkedHashMap;
import java.util.List;
import java.util.Map;

/** Everything persisted about one player (GDD 20). */
public final class PlayerState {

    public String playerId = "local";
    public int level = 1;
    public int xp;

    public final Map<Currency, Integer> currencies = new EnumMap<Currency, Integer>(Currency.class);

    public final Map<String, OwnedGear> gear = new LinkedHashMap<String, OwnedGear>();
    public final Map<String, OwnedAngler> anglers = new LinkedHashMap<String, OwnedAngler>();
    public final Loadout loadout = new Loadout();

    public final Map<String, CodexEntry> codex = new LinkedHashMap<String, CodexEntry>();
    public final List<FishRecord> recentCatches = new ArrayList<FishRecord>();

    public String currentSpotId;
    public AutoStrategy autoStrategy = AutoStrategy.BALANCED;
    public boolean autoEnabled = true;

    public int talentPull;
    public int talentSafety;
    public int talentLuck;
    public int talentOffline;
    public int talentPoints;

    /** Wall-clock millis of the last settled session; the offline model integrates from here. */
    public long lastSeenAt;

    /** Bounded audit trail kept on the client for support; the server keeps the real one. */
    public final List<LedgerEntry> ledger = new ArrayList<LedgerEntry>();

    public static final int RECENT_CATCH_LIMIT = 50;
    public static final int LEDGER_LIMIT = 200;

    public int get(Currency c) {
        Integer v = currencies.get(c);
        return v == null ? 0 : v;
    }

    /**
     * The only way currency changes. Refuses to spend more than the player has, and records the
     * mutation so the economy can be audited (GDD 19).
     *
     * @return true if applied
     */
    public boolean applyCurrency(Currency c, int delta, String reason, String txId) {
        int current = get(c);
        if (delta < 0 && current + delta < 0) {
            return false;
        }
        for (int i = 0; i < ledger.size(); i++) {
            if (txId != null && txId.equals(ledger.get(i).txId)) {
                return false; // idempotency: this transaction was already applied
            }
        }
        currencies.put(c, current + delta);

        LedgerEntry e = new LedgerEntry();
        e.at = lastSeenAt;
        e.currency = c;
        e.amount = delta;
        e.reason = reason;
        e.txId = txId;
        ledger.add(e);
        while (ledger.size() > LEDGER_LIMIT) ledger.remove(0);
        return true;
    }

    public boolean canAfford(Currency c, int cost) {
        return get(c) >= cost;
    }

    /** XP curve: deliberately gentle early and steep late (GDD 8). */
    public static int xpForLevel(int level) {
        return (int) (60 * Math.pow(level, 1.65));
    }

    /** @return number of levels gained */
    public int addXp(int amount) {
        xp += amount;
        int gained = 0;
        while (xp >= xpForLevel(level)) {
            xp -= xpForLevel(level);
            level++;
            talentPoints++;
            gained++;
        }
        return gained;
    }

    public CodexEntry codexFor(String speciesId) {
        CodexEntry e = codex.get(speciesId);
        if (e == null) {
            e = new CodexEntry();
            e.speciesId = speciesId;
            codex.put(speciesId, e);
        }
        return e;
    }

    public int discoveredSpecies() {
        int n = 0;
        for (CodexEntry e : codex.values()) {
            if (e.isDiscovered()) n++;
        }
        return n;
    }

    public void recordCatch(FishRecord record) {
        CodexEntry e = codexFor(record.speciesId);
        e.caughtCount++;
        if (record.weight > e.heaviest) {
            e.heaviest = record.weight;
            record.personalBest = e.caughtCount > 1;
        }
        if (e.firstCaughtAt == 0) e.firstCaughtAt = record.caughtAt;
        e.lastCaughtAt = record.caughtAt;

        recentCatches.add(0, record);
        while (recentCatches.size() > RECENT_CATCH_LIMIT) {
            recentCatches.remove(recentCatches.size() - 1);
        }
    }
}
