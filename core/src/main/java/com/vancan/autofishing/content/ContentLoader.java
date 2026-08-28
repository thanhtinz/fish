package com.vancan.autofishing.content;

import com.badlogic.gdx.utils.JsonReader;
import com.badlogic.gdx.utils.JsonValue;
import com.vancan.autofishing.sim.Archetype;
import com.vancan.autofishing.sim.Rarity;
import com.vancan.autofishing.sim.SimConfig;
import com.vancan.autofishing.sim.SkillDef;
import com.vancan.autofishing.sim.SpeciesDef;

/**
 * Reads the designer-authored JSON tables into a {@link GameContent} registry.
 *
 * <p>Fields are read explicitly rather than through reflection: GWT cannot reflect, and an
 * explicit read is also the place where a renamed or missing field turns into a clear error
 * instead of a silently-zero stat.
 */
public final class ContentLoader {

    private final JsonReader json = new JsonReader();

    public GameContent load(TextSource source) {
        GameContent content = new GameContent();
        content.simConfig = readSimConfig(source.read("sim_config.json"));
        readSpecies(content, source.read("species.json"));
        readSkills(content, source.read("skills.json"));
        readGear(content, source.read("gear.json"));
        readAnglers(content, source.read("anglers.json"));
        readSpots(content, source.read("spots.json"));

        ContentValidator.Report report = ContentValidator.validate(content);
        if (!report.ok()) {
            throw new IllegalStateException("Content validation failed:\n" + report);
        }
        return content;
    }

    private SimConfig readSimConfig(String text) {
        JsonValue v = json.parse(text);
        SimConfig c = new SimConfig();
        c.tickSeconds = v.getFloat("tickSeconds", c.tickSeconds);
        c.baseBiteIntervalSeconds = v.getFloat("baseBiteIntervalSeconds", c.baseBiteIntervalSeconds);
        c.biteWindowSeconds = v.getFloat("biteWindowSeconds", c.biteWindowSeconds);
        c.searchTimeoutSeconds = v.getFloat("searchTimeoutSeconds", c.searchTimeoutSeconds);
        c.damageCoefficient = v.getFloat("damageCoefficient", c.damageCoefficient);
        c.critMultiplier = v.getFloat("critMultiplier", c.critMultiplier);
        c.staminaDamageCoefficient = v.getFloat("staminaDamageCoefficient", c.staminaDamageCoefficient);
        c.pullTensionCoefficient = v.getFloat("pullTensionCoefficient", c.pullTensionCoefficient);
        c.safeTensionRatio = v.getFloat("safeTensionRatio", c.safeTensionRatio);
        c.wearRate = v.getFloat("wearRate", c.wearRate);
        c.lineResponse = v.getFloat("lineResponse", c.lineResponse);
        c.elasticityRelief = v.getFloat("elasticityRelief", c.elasticityRelief);
        c.phaseShockFactor = v.getFloat("phaseShockFactor", c.phaseShockFactor);
        c.landingThreshold = v.getFloat("landingThreshold", c.landingThreshold);
        c.castDistanceRatio = v.getFloat("castDistanceRatio", c.castDistanceRatio);
        c.reelSpeedCoefficient = v.getFloat("reelSpeedCoefficient", c.reelSpeedCoefficient);
        c.fightTimeoutSeconds = v.getFloat("fightTimeoutSeconds", c.fightTimeoutSeconds);
        c.fishStaminaRecovery = v.getFloat("fishStaminaRecovery", c.fishStaminaRecovery);
        c.fatigueForceFloor = v.getFloat("fatigueForceFloor", c.fatigueForceFloor);
        c.offlinePenalty = v.getFloat("offlinePenalty", c.offlinePenalty);
        c.offlineCapHours = v.getFloat("offlineCapHours", c.offlineCapHours);
        return c;
    }

    private void readSpecies(GameContent content, String text) {
        for (JsonValue v = json.parse(text).get("species").child; v != null; v = v.next) {
            SpeciesDef s = new SpeciesDef();
            s.id = require(v, "id");
            s.name = require(v, "name");
            s.rarity = Rarity.fromId(v.getString("rarity", "COMMON"));
            s.archetype = Archetype.fromId(v.getString("archetype", "RUNNER"));
            s.minWeight = v.getFloat("minWeight", s.minWeight);
            s.maxWeight = v.getFloat("maxWeight", s.maxWeight);
            s.weightBias = v.getFloat("weightBias", s.weightBias);
            s.baseHp = v.getFloat("baseHp", s.baseHp);
            s.hpPerKg = v.getFloat("hpPerKg", s.hpPerKg);
            s.baseStamina = v.getFloat("baseStamina", s.baseStamina);
            s.staminaPerKg = v.getFloat("staminaPerKg", s.staminaPerKg);
            s.basePower = v.getFloat("basePower", s.basePower);
            s.powerPerKg = v.getFloat("powerPerKg", s.powerPerKg);
            s.baseSpeed = v.getFloat("baseSpeed", s.baseSpeed);
            s.speedPerKg = v.getFloat("speedPerKg", s.speedPerKg);
            s.escapeRate = v.getFloat("escapeRate", s.escapeRate);
            s.hookDifficulty = v.getFloat("hookDifficulty", s.hookDifficulty);
            s.depth = v.getFloat("depth", s.depth);
            s.baseValue = v.getInt("baseValue", s.baseValue);
            s.baseXp = v.getInt("baseXp", s.baseXp);
            s.description = v.getString("description", "");
            putUnique(content.species.containsKey(s.id), "species", s.id);
            content.species.put(s.id, s);
        }
    }

    private void readSkills(GameContent content, String text) {
        for (JsonValue v = json.parse(text).get("skills").child; v != null; v = v.next) {
            SkillDef s = new SkillDef();
            s.id = require(v, "id");
            s.name = require(v, "name");
            s.effect = SkillDef.Effect.valueOf(v.getString("effect", "BURST"));
            s.magnitude = v.getFloat("magnitude", s.magnitude);
            s.durationSeconds = v.getFloat("durationSeconds", s.durationSeconds);
            s.cooldownSeconds = v.getFloat("cooldownSeconds", s.cooldownSeconds);
            s.description = v.getString("description", "");
            putUnique(content.skills.containsKey(s.id), "skill", s.id);
            content.skills.put(s.id, s);
        }
    }

    private void readGear(GameContent content, String text) {
        for (JsonValue v = json.parse(text).get("gear").child; v != null; v = v.next) {
            GearTemplate g = new GearTemplate();
            g.id = require(v, "id");
            g.name = require(v, "name");
            g.slot = GearSlot.fromId(v.getString("slot", "ROD"));
            g.tier = v.getInt("tier", 1);
            g.rarity = Rarity.fromId(v.getString("rarity", "COMMON"));
            g.description = v.getString("description", "");
            JsonValue stats = v.get("stats");
            if (stats != null) {
                for (JsonValue s = stats.child; s != null; s = s.next) {
                    if ("speciesBias".equals(s.name)) {
                        g.speciesBias = s.asString();
                    } else {
                        g.stats.put(s.name, s.asFloat());
                    }
                }
            }
            putUnique(content.gear.containsKey(g.id), "gear", g.id);
            content.gear.put(g.id, g);
        }
    }

    private void readAnglers(GameContent content, String text) {
        for (JsonValue v = json.parse(text).get("anglers").child; v != null; v = v.next) {
            AnglerDef a = new AnglerDef();
            a.id = require(v, "id");
            a.name = require(v, "name");
            a.role = TeamRole.fromId(v.getString("role", "STRIKER"));
            a.rarity = Rarity.fromId(v.getString("rarity", "COMMON"));
            a.signatureSkill = v.getString("signatureSkill", null);
            a.description = v.getString("description", "");
            JsonValue stats = v.get("statsPerLevel");
            if (stats != null) {
                for (JsonValue s = stats.child; s != null; s = s.next) {
                    a.statsPerLevel.put(s.name, s.asFloat());
                }
            }
            putUnique(content.anglers.containsKey(a.id), "angler", a.id);
            content.anglers.put(a.id, a);
        }
    }

    private void readSpots(GameContent content, String text) {
        for (JsonValue v = json.parse(text).get("spots").child; v != null; v = v.next) {
            SpotDef s = new SpotDef();
            s.id = require(v, "id");
            s.name = require(v, "name");
            s.theme = v.getString("theme", "");
            s.description = v.getString("description", "");
            s.unlockLevel = v.getInt("unlockLevel", 1);
            s.tier = v.getInt("tier", 1);
            s.weather = v.getString("weather", "CALM");
            s.bossSpecies = v.getString("bossSpecies", null);
            JsonValue pool = v.get("fishPool");
            if (pool != null) {
                for (JsonValue p = pool.child; p != null; p = p.next) {
                    SpotDef.PoolEntry e = new SpotDef.PoolEntry();
                    e.speciesId = require(p, "species");
                    e.weight = p.getFloat("weight", 1f);
                    s.fishPool.add(e);
                }
            }
            putUnique(content.spots.containsKey(s.id), "spot", s.id);
            content.spots.put(s.id, s);
        }
    }

    private static String require(JsonValue v, String field) {
        String value = v.getString(field, null);
        if (value == null || value.isEmpty()) {
            throw new IllegalStateException("Missing required field '" + field + "' in " + v);
        }
        return value;
    }

    private static void putUnique(boolean alreadyPresent, String kind, String id) {
        if (alreadyPresent) {
            throw new IllegalStateException("Duplicate " + kind + " id '" + id + "'");
        }
    }
}
