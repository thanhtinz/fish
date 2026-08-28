package com.vancan.autofishing.content;

import com.vancan.autofishing.sim.SpeciesDef;

import java.util.ArrayList;
import java.util.List;

/**
 * Content validation from GDD 22.
 *
 * <p>Data bugs in a live game are expensive and quiet: a dangling species id in a spot pool only
 * shows up as a crash for the players who reach that zone. This runs over the whole registry at
 * load time (and in CI via ContentTest) so those never reach a build.
 */
public final class ContentValidator {

    private ContentValidator() {
    }

    public static final class Report {
        public final List<String> errors = new ArrayList<String>();
        public final List<String> warnings = new ArrayList<String>();

        public boolean ok() {
            return errors.isEmpty();
        }

        @Override
        public String toString() {
            StringBuilder sb = new StringBuilder();
            for (String e : errors) sb.append("ERROR   ").append(e).append('\n');
            for (String w : warnings) sb.append("WARNING ").append(w).append('\n');
            if (sb.length() == 0) sb.append("content OK");
            return sb.toString();
        }
    }

    public static Report validate(GameContent c) {
        Report r = new Report();

        if (c.species.isEmpty()) r.errors.add("no species defined");
        if (c.spots.isEmpty()) r.errors.add("no spots defined");

        for (SpeciesDef s : c.species.values()) {
            String at = "species '" + s.id + "'";
            if (s.name == null || s.name.isEmpty()) r.errors.add(at + " has no name");
            if (s.minWeight <= 0) r.errors.add(at + " minWeight must be > 0");
            if (s.maxWeight < s.minWeight) r.errors.add(at + " maxWeight < minWeight");
            if (s.baseHp <= 0 || s.baseStamina <= 0) r.errors.add(at + " needs positive hp/stamina");
            if (s.basePower <= 0) r.errors.add(at + " needs positive power");
            if (s.escapeRate < 0 || s.escapeRate > 1) r.errors.add(at + " escapeRate out of [0,1]");
            if (s.hookDifficulty <= 0) r.errors.add(at + " hookDifficulty must be > 0");
            if (s.baseValue <= 0) r.warnings.add(at + " is worth no gold");
        }

        for (SpotDef spot : c.spots.values()) {
            String at = "spot '" + spot.id + "'";
            if (spot.fishPool.isEmpty()) {
                r.errors.add(at + " has an empty fish pool");
            }
            float total = 0f;
            for (SpotDef.PoolEntry e : spot.fishPool) {
                if (!c.species.containsKey(e.speciesId)) {
                    r.errors.add(at + " references unknown species '" + e.speciesId + "'");
                }
                if (e.weight <= 0) r.errors.add(at + " gives species '" + e.speciesId + "' a non-positive weight");
                total += e.weight;
            }
            if (total <= 0) r.errors.add(at + " pool weights sum to zero");
            if (spot.bossSpecies != null && !c.species.containsKey(spot.bossSpecies)) {
                r.errors.add(at + " references unknown boss '" + spot.bossSpecies + "'");
            }
            if (spot.unlockLevel < 1) r.errors.add(at + " unlockLevel must be >= 1");
        }

        // Progression sanity: zones must unlock in a strictly increasing order, otherwise the map
        // screen shows a later zone as reachable before an earlier one.
        int previousUnlock = 0;
        String previousId = null;
        for (SpotDef spot : c.spots.values()) {
            if (spot.unlockLevel < previousUnlock) {
                r.errors.add("spot '" + spot.id + "' unlocks at " + spot.unlockLevel
                        + " but is listed after '" + previousId + "' which unlocks at " + previousUnlock);
            }
            previousUnlock = spot.unlockLevel;
            previousId = spot.id;
        }

        for (GearTemplate g : c.gear.values()) {
            String at = "gear '" + g.id + "'";
            if (g.stats.isEmpty()) r.warnings.add(at + " has no stats");
            if (g.tier < 1) r.errors.add(at + " tier must be >= 1");
            if (g.speciesBias != null && !c.species.containsKey(g.speciesBias)) {
                r.errors.add(at + " biases towards unknown species '" + g.speciesBias + "'");
            }
        }

        for (AnglerDef a : c.anglers.values()) {
            String at = "angler '" + a.id + "'";
            if (a.statsPerLevel.isEmpty()) r.warnings.add(at + " contributes nothing");
            if (a.signatureSkill != null && !c.skills.containsKey(a.signatureSkill)) {
                r.errors.add(at + " references unknown skill '" + a.signatureSkill + "'");
            }
        }

        for (com.vancan.autofishing.sim.SkillDef s : c.skills.values()) {
            String at = "skill '" + s.id + "'";
            if (s.cooldownSeconds <= 0) r.errors.add(at + " cooldown must be > 0");
            if (s.durationSeconds <= 0) r.errors.add(at + " duration must be > 0");
            if (s.durationSeconds >= s.cooldownSeconds) {
                r.errors.add(at + " has 100% uptime (duration >= cooldown)");
            }
        }

        // Every slot must have something a new player can equip, or the first cast crashes.
        for (GearSlot slot : GearSlot.values()) {
            if (c.starterGear(slot) == null) {
                r.errors.add("no gear defined for slot " + slot);
            }
        }

        return r;
    }
}
