package com.vancan.autofishing;

import com.vancan.autofishing.content.ContentValidator;
import com.vancan.autofishing.content.GameContent;
import com.vancan.autofishing.content.GearSlot;
import com.vancan.autofishing.content.SpotDef;
import com.vancan.autofishing.sim.Rarity;
import com.vancan.autofishing.sim.SpeciesDef;
import org.junit.jupiter.api.Test;

import static org.junit.jupiter.api.Assertions.assertFalse;
import static org.junit.jupiter.api.Assertions.assertNotNull;
import static org.junit.jupiter.api.Assertions.assertTrue;

/** GDD 22: the shipped content tables must load and pass validation. */
class ContentTest {

    @Test
    void shippedContentLoadsAndValidates() {
        GameContent c = TestContent.get();
        ContentValidator.Report report = ContentValidator.validate(c);
        assertTrue(report.ok(), "content validation failed:\n" + report);
    }

    @Test
    void everySpotPoolResolves() {
        GameContent c = TestContent.get();
        assertFalse(c.spots.isEmpty());
        for (SpotDef spot : c.spots.values()) {
            assertNotNull(spot.buildTable(c.species), spot.id);
            assertFalse(spot.buildTable(c.species).isEmpty(), spot.id + " has an empty table");
        }
    }

    @Test
    void everyRarityTierHasAtLeastOneSpecies() {
        GameContent c = TestContent.get();
        for (Rarity r : Rarity.values()) {
            boolean found = false;
            for (SpeciesDef s : c.species.values()) {
                if (s.rarity == r) {
                    found = true;
                    break;
                }
            }
            assertTrue(found, "no species defined for rarity " + r
                    + " - the codex would show an unreachable tier");
        }
    }

    @Test
    void everySlotHasStarterGear() {
        GameContent c = TestContent.get();
        for (GearSlot slot : GearSlot.values()) {
            assertNotNull(c.starterGear(slot), "no starter gear for " + slot);
        }
    }

    @Test
    void validatorRejectsADanglingSpeciesReference() {
        GameContent c = TestContent.get();
        GameContent broken = new GameContent();
        broken.species.putAll(c.species);
        broken.gear.putAll(c.gear);
        broken.skills.putAll(c.skills);
        broken.anglers.putAll(c.anglers);

        SpotDef bad = new SpotDef();
        bad.id = "broken_spot";
        bad.name = "Broken";
        SpotDef.PoolEntry e = new SpotDef.PoolEntry();
        e.speciesId = "does_not_exist";
        e.weight = 1f;
        bad.fishPool.add(e);
        broken.spots.put(bad.id, bad);

        ContentValidator.Report report = ContentValidator.validate(broken);
        assertFalse(report.ok(), "validator should have caught the dangling species id");
    }
}
