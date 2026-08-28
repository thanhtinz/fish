package com.vancan.autofishing.content;

import com.vancan.autofishing.sim.SimConfig;
import com.vancan.autofishing.sim.SkillDef;
import com.vancan.autofishing.sim.SpeciesDef;

import java.util.ArrayList;
import java.util.LinkedHashMap;
import java.util.List;
import java.util.Map;

/** In-memory registry of everything the designers authored. Immutable once loaded. */
public final class GameContent {

    public SimConfig simConfig = new SimConfig();

    public final Map<String, SpeciesDef> species = new LinkedHashMap<String, SpeciesDef>();
    public final Map<String, SpotDef> spots = new LinkedHashMap<String, SpotDef>();
    public final Map<String, GearTemplate> gear = new LinkedHashMap<String, GearTemplate>();
    public final Map<String, SkillDef> skills = new LinkedHashMap<String, SkillDef>();
    public final Map<String, AnglerDef> anglers = new LinkedHashMap<String, AnglerDef>();

    public List<SpotDef> spotsInOrder() {
        return new ArrayList<SpotDef>(spots.values());
    }

    public List<GearTemplate> gearForSlot(GearSlot slot) {
        List<GearTemplate> out = new ArrayList<GearTemplate>();
        for (GearTemplate g : gear.values()) {
            if (g.slot == slot) out.add(g);
        }
        return out;
    }

    /** First (lowest-tier) item in a slot; used to seed a new player's loadout. */
    public GearTemplate starterGear(GearSlot slot) {
        GearTemplate best = null;
        for (GearTemplate g : gear.values()) {
            if (g.slot == slot && (best == null || g.tier < best.tier)) best = g;
        }
        return best;
    }
}
