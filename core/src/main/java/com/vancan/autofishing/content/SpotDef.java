package com.vancan.autofishing.content;

import com.vancan.autofishing.sim.EncounterTable;
import com.vancan.autofishing.sim.SpeciesDef;

import java.util.ArrayList;
import java.util.List;
import java.util.Map;

/** A fishing spot / zone (GDD 10). */
public final class SpotDef {

    public static final class PoolEntry {
        public String speciesId;
        public float weight;
    }

    public String id;
    public String name;
    public String theme = "";
    public String description = "";
    public int unlockLevel = 1;
    public int tier = 1;
    public String weather = "CALM";
    public String bossSpecies;
    public final List<PoolEntry> fishPool = new ArrayList<PoolEntry>();

    /** Builds the runtime encounter table, resolving species ids against the registry. */
    public EncounterTable buildTable(Map<String, SpeciesDef> species) {
        EncounterTable table = new EncounterTable();
        for (int i = 0; i < fishPool.size(); i++) {
            PoolEntry e = fishPool.get(i);
            SpeciesDef def = species.get(e.speciesId);
            if (def == null) {
                throw new IllegalStateException(
                        "Spot '" + id + "' references unknown species '" + e.speciesId + "'");
            }
            table.add(def, e.weight);
        }
        return table;
    }

    /** Boss encounters are a separate single-entry table so they cannot roll a trash fish. */
    public EncounterTable buildBossTable(Map<String, SpeciesDef> species) {
        if (bossSpecies == null) return null;
        SpeciesDef def = species.get(bossSpecies);
        if (def == null) {
            throw new IllegalStateException(
                    "Spot '" + id + "' references unknown boss '" + bossSpecies + "'");
        }
        EncounterTable table = new EncounterTable();
        table.add(def, 1f);
        return table;
    }
}
