package com.vancan.autofishing;

import com.vancan.autofishing.auto.AutoStrategy;
import com.vancan.autofishing.content.GameContent;
import com.vancan.autofishing.content.GearSlot;
import com.vancan.autofishing.meta.CatchResolver;
import com.vancan.autofishing.meta.Currency;
import com.vancan.autofishing.meta.FishRecord;
import com.vancan.autofishing.meta.LoadoutResolver;
import com.vancan.autofishing.meta.OfflineSettlement;
import com.vancan.autofishing.meta.PlayerFactory;
import com.vancan.autofishing.meta.PlayerState;
import com.vancan.autofishing.meta.SaveGame;
import com.vancan.autofishing.sim.BuildStats;
import com.vancan.autofishing.sim.FishState;
import com.vancan.autofishing.sim.Rng;
import org.junit.jupiter.api.Test;

import static org.junit.jupiter.api.Assertions.assertEquals;
import static org.junit.jupiter.api.Assertions.assertFalse;
import static org.junit.jupiter.api.Assertions.assertNotNull;
import static org.junit.jupiter.api.Assertions.assertTrue;

class MetaTest {

    private GameContent content() {
        return TestContent.get();
    }

    @Test
    void newPlayerCanImmediatelyFish() {
        GameContent c = content();
        PlayerState p = PlayerFactory.newPlayer(c, 1000L);
        assertNotNull(p.currentSpotId, "new player has no spot selected");
        for (GearSlot slot : GearSlot.values()) {
            assertNotNull(p.loadout.equipped.get(slot), "new player has nothing in " + slot);
        }
        BuildStats b = LoadoutResolver.resolve(p, c);
        assertTrue(b.lineStrength > 0, "resolved build has no line strength");
        assertTrue(b.hookRate > 0, "resolved build cannot hook anything");
        assertTrue(b.pullSpeed > 0, "resolved build cannot reel");
    }

    @Test
    void saveRoundTripsEverythingThatMatters() {
        GameContent c = content();
        PlayerState p = PlayerFactory.newPlayer(c, 4242L);
        p.level = 17;
        p.xp = 350;
        p.talentLuck = 4;
        p.talentPoints = 2;
        p.autoStrategy = AutoStrategy.RARE_HUNTER;
        p.autoEnabled = false;
        p.applyCurrency(Currency.GOLD, 999, "test", "tx-1");

        FishState fish = FishState.of(c.species.get("ca_chep"), new Rng(3));
        FishRecord record = CatchResolver.resolve(fish, p.currentSpotId, 5555L);
        p.recordCatch(record);

        PlayerState loaded = SaveGame.load(SaveGame.save(p));

        assertEquals(p.level, loaded.level);
        assertEquals(p.xp, loaded.xp);
        assertEquals(p.get(Currency.GOLD), loaded.get(Currency.GOLD));
        assertEquals(p.autoStrategy, loaded.autoStrategy);
        assertEquals(p.autoEnabled, loaded.autoEnabled);
        assertEquals(p.talentLuck, loaded.talentLuck);
        assertEquals(p.gear.size(), loaded.gear.size());
        assertEquals(p.loadout.team, loaded.loadout.team);
        assertEquals(p.loadout.skills, loaded.loadout.skills);
        assertEquals(p.loadout.equipped, loaded.loadout.equipped);
        assertEquals(p.discoveredSpecies(), loaded.discoveredSpecies());

        // The resolved build is what actually reaches the simulation, so compare that too.
        BuildStats a = LoadoutResolver.resolve(p, c);
        BuildStats b = LoadoutResolver.resolve(loaded, c);
        assertEquals(a.rodPower, b.rodPower, 1e-4f);
        assertEquals(a.lineStrength, b.lineStrength, 1e-4f);
        assertEquals(a.luck, b.luck, 1e-4f);
    }

    @Test
    void currencyCannotGoNegative() {
        PlayerState p = PlayerFactory.newPlayer(content(), 0L);
        int before = p.get(Currency.GOLD);
        assertFalse(p.applyCurrency(Currency.GOLD, -(before + 1), "overspend", "tx-over"));
        assertEquals(before, p.get(Currency.GOLD), "an unaffordable spend still moved the balance");
    }

    @Test
    void repeatedTransactionIdIsNotAppliedTwice() {
        // GDD 19: settlements carry an idempotency key so a retried claim cannot pay twice.
        PlayerState p = PlayerFactory.newPlayer(content(), 0L);
        int before = p.get(Currency.GOLD);
        assertTrue(p.applyCurrency(Currency.GOLD, 500, "reward", "tx-dup"));
        assertFalse(p.applyCurrency(Currency.GOLD, 500, "reward", "tx-dup"));
        assertEquals(before + 500, p.get(Currency.GOLD), "duplicate transaction paid out twice");
    }

    @Test
    void levellingConsumesXpAndGrantsTalentPoints() {
        PlayerState p = PlayerFactory.newPlayer(content(), 0L);
        int levels = p.addXp(100000);
        assertTrue(levels > 0, "a large XP grant produced no levels");
        assertEquals(levels, p.talentPoints, "talent points did not track levels gained");
        assertTrue(p.xp < PlayerState.xpForLevel(p.level), "leftover XP exceeds the next level");
    }

    @Test
    void offlineRewardsAreCappedAndIdempotent() {
        GameContent c = content();
        PlayerState p = PlayerFactory.newPlayer(c, 0L);
        BuildStats build = LoadoutResolver.resolve(p, c);

        long thirtyHours = 30L * 3600_000L;
        OfflineSettlement.Result r = OfflineSettlement.settle(p, c, build, thirtyHours, 1000L);
        assertNotNull(r, "no settlement produced for 30 hours away");
        assertTrue(r.capped, "30 hours offline was not capped");
        assertTrue(r.hoursCredited <= c.simConfig.offlineCapHours * build.offlineEfficiency + 0.01f,
                "credited more hours than the cap allows");

        int goldBefore = p.get(Currency.GOLD);
        assertTrue(OfflineSettlement.apply(p, r));
        assertEquals(goldBefore + r.gold, p.get(Currency.GOLD));

        assertFalse(OfflineSettlement.apply(p, r), "the same settlement paid out twice");
        assertEquals(goldBefore + r.gold, p.get(Currency.GOLD));
    }

    @Test
    void shortAbsenceEarnsProportionallyLessThanALongOne() {
        GameContent c = content();
        PlayerState p = PlayerFactory.newPlayer(c, 0L);
        BuildStats build = LoadoutResolver.resolve(p, c);

        OfflineSettlement.Result one = OfflineSettlement.settle(p, c, build, 3600_000L, 1L);
        OfflineSettlement.Result four = OfflineSettlement.settle(p, c, build, 4 * 3600_000L, 2L);
        assertNotNull(one);
        assertNotNull(four);
        assertTrue(four.gold > one.gold, "four hours offline did not beat one hour");
        assertTrue(four.gold <= one.gold * 5,
                "offline rewards are growing faster than linearly, which breaks the sink model");
    }

    @Test
    void offlineYieldStaysBelowActivePlay() {
        // Idle must supplement play, not replace it (GDD 12: offlinePenalty exists for this).
        GameContent c = content();
        PlayerState p = PlayerFactory.newPlayer(c, 0L);
        BuildStats build = LoadoutResolver.resolve(p, c);
        OfflineSettlement.Result r = OfflineSettlement.settle(p, c, build, 3600_000L, 1L);
        assertNotNull(r);

        float offlineCastsPerHour = r.casts;
        float activeCastsPerHour = 3600f / OfflineSettlement.secondsPerCast(c.simConfig, build);
        assertTrue(r.catches < activeCastsPerHour * build.hookRate,
                "offline catches (" + r.catches + ") match or beat what active play would land");
        assertTrue(offlineCastsPerHour > 0, "offline produced no casts at all");
    }

    @Test
    void heavierFishAreWorthMoreButNotRunawayMore() {
        GameContent c = content();
        com.vancan.autofishing.sim.SpeciesDef def = c.species.get("ca_chep");

        FishState light = FishState.of(def, new Rng(1));
        light.weight = def.minWeight;
        FishState heavy = FishState.of(def, new Rng(1));
        heavy.weight = def.maxWeight;

        int lightGold = CatchResolver.resolve(light, "z1", 0L).goldValue;
        int heavyGold = CatchResolver.resolve(heavy, "z1", 0L).goldValue;

        assertTrue(heavyGold > lightGold, "a record fish is worth no more than a small one");
        assertTrue(heavyGold < lightGold * 12,
                "weight value scaling is runaway (" + lightGold + " -> " + heavyGold
                        + "); one lucky catch would outweigh an hour of play");
    }

    @Test
    void codexTracksHeaviestPerSpecies() {
        GameContent c = content();
        PlayerState p = PlayerFactory.newPlayer(c, 0L);
        com.vancan.autofishing.sim.SpeciesDef def = c.species.get("ca_ro");

        FishState small = FishState.of(def, new Rng(1));
        small.weight = 0.5f;
        p.recordCatch(CatchResolver.resolve(small, "z1", 10L));

        FishState big = FishState.of(def, new Rng(2));
        big.weight = 1.4f;
        p.recordCatch(CatchResolver.resolve(big, "z1", 20L));

        FishState mid = FishState.of(def, new Rng(3));
        mid.weight = 0.9f;
        p.recordCatch(CatchResolver.resolve(mid, "z1", 30L));

        assertEquals(3, p.codexFor("ca_ro").caughtCount);
        assertEquals(1.4f, p.codexFor("ca_ro").heaviest, 1e-4f);
        assertEquals(1, p.discoveredSpecies());
    }
}
