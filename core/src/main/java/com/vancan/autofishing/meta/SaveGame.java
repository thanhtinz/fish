package com.vancan.autofishing.meta;

import com.badlogic.gdx.utils.JsonReader;
import com.badlogic.gdx.utils.JsonValue;
import com.vancan.autofishing.auto.AutoStrategy;
import com.vancan.autofishing.content.GearSlot;
import com.vancan.autofishing.sim.Rarity;

/**
 * Save serialisation.
 *
 * <p>Written by hand instead of via libGDX's reflective {@code Json}: GWT has no reflection, and a
 * hand-written format also means a field rename cannot silently drop a player's inventory. Reads
 * are tolerant of missing fields so an older save still loads after a content update.
 */
public final class SaveGame {

    /** Bumped when the shape changes incompatibly; {@link #load} refuses newer saves. */
    public static final int VERSION = 1;

    private SaveGame() {
    }

    public static String save(PlayerState p) {
        StringBuilder sb = new StringBuilder(1024);
        sb.append("{\"version\":").append(VERSION);
        sb.append(",\"playerId\":").append(quote(p.playerId));
        sb.append(",\"level\":").append(p.level);
        sb.append(",\"xp\":").append(p.xp);
        sb.append(",\"lastSeenAt\":").append(p.lastSeenAt);
        sb.append(",\"currentSpotId\":").append(quote(p.currentSpotId));
        sb.append(",\"autoStrategy\":").append(quote(p.autoStrategy.name()));
        sb.append(",\"autoEnabled\":").append(p.autoEnabled);
        sb.append(",\"talentPull\":").append(p.talentPull);
        sb.append(",\"talentSafety\":").append(p.talentSafety);
        sb.append(",\"talentLuck\":").append(p.talentLuck);
        sb.append(",\"talentOffline\":").append(p.talentOffline);
        sb.append(",\"talentPoints\":").append(p.talentPoints);

        sb.append(",\"currencies\":{");
        boolean first = true;
        for (Currency c : Currency.values()) {
            if (!first) sb.append(',');
            first = false;
            sb.append(quote(c.name())).append(':').append(p.get(c));
        }
        sb.append('}');

        sb.append(",\"gear\":[");
        first = true;
        for (OwnedGear g : p.gear.values()) {
            if (!first) sb.append(',');
            first = false;
            sb.append("{\"id\":").append(quote(g.id))
              .append(",\"templateId\":").append(quote(g.templateId))
              .append(",\"level\":").append(g.level).append('}');
        }
        sb.append(']');

        sb.append(",\"anglers\":[");
        first = true;
        for (OwnedAngler a : p.anglers.values()) {
            if (!first) sb.append(',');
            first = false;
            sb.append("{\"id\":").append(quote(a.id))
              .append(",\"defId\":").append(quote(a.defId))
              .append(",\"level\":").append(a.level)
              .append(",\"stars\":").append(a.stars).append('}');
        }
        sb.append(']');

        sb.append(",\"equipped\":{");
        first = true;
        for (GearSlot slot : GearSlot.values()) {
            String v = p.loadout.equipped.get(slot);
            if (v == null) continue;
            if (!first) sb.append(',');
            first = false;
            sb.append(quote(slot.name())).append(':').append(quote(v));
        }
        sb.append('}');

        sb.append(",\"team\":").append(stringArray(p.loadout.team));
        sb.append(",\"skills\":").append(stringArray(p.loadout.skills));

        sb.append(",\"codex\":[");
        first = true;
        for (CodexEntry e : p.codex.values()) {
            if (!first) sb.append(',');
            first = false;
            sb.append("{\"speciesId\":").append(quote(e.speciesId))
              .append(",\"caughtCount\":").append(e.caughtCount)
              .append(",\"heaviest\":").append(e.heaviest)
              .append(",\"firstCaughtAt\":").append(e.firstCaughtAt)
              .append(",\"lastCaughtAt\":").append(e.lastCaughtAt).append('}');
        }
        sb.append(']');

        sb.append('}');
        return sb.toString();
    }

    public static PlayerState load(String text) {
        JsonValue v = new JsonReader().parse(text);
        int version = v.getInt("version", 0);
        if (version > VERSION) {
            throw new IllegalStateException("Save was written by a newer build (version "
                    + version + " > " + VERSION + ")");
        }

        PlayerState p = new PlayerState();
        p.playerId = v.getString("playerId", "local");
        p.level = v.getInt("level", 1);
        p.xp = v.getInt("xp", 0);
        p.lastSeenAt = v.getLong("lastSeenAt", 0L);
        p.currentSpotId = v.getString("currentSpotId", null);
        p.autoStrategy = parseStrategy(v.getString("autoStrategy", "BALANCED"));
        p.autoEnabled = v.getBoolean("autoEnabled", true);
        p.talentPull = v.getInt("talentPull", 0);
        p.talentSafety = v.getInt("talentSafety", 0);
        p.talentLuck = v.getInt("talentLuck", 0);
        p.talentOffline = v.getInt("talentOffline", 0);
        p.talentPoints = v.getInt("talentPoints", 0);

        JsonValue currencies = v.get("currencies");
        if (currencies != null) {
            for (JsonValue c = currencies.child; c != null; c = c.next) {
                for (Currency cur : Currency.values()) {
                    if (cur.name().equals(c.name)) p.currencies.put(cur, c.asInt());
                }
            }
        }

        JsonValue gear = v.get("gear");
        if (gear != null) {
            for (JsonValue g = gear.child; g != null; g = g.next) {
                OwnedGear owned = new OwnedGear(g.getString("id"), g.getString("templateId"),
                        g.getInt("level", 1));
                p.gear.put(owned.id, owned);
            }
        }

        JsonValue anglers = v.get("anglers");
        if (anglers != null) {
            for (JsonValue a = anglers.child; a != null; a = a.next) {
                OwnedAngler owned = new OwnedAngler(a.getString("id"), a.getString("defId"),
                        a.getInt("level", 1), a.getInt("stars", 1));
                p.anglers.put(owned.id, owned);
            }
        }

        JsonValue equipped = v.get("equipped");
        if (equipped != null) {
            for (JsonValue e = equipped.child; e != null; e = e.next) {
                // Skip slots the current build no longer defines rather than failing the load.
                for (GearSlot slot : GearSlot.values()) {
                    if (slot.name().equals(e.name)) p.loadout.equipped.put(slot, e.asString());
                }
            }
        }

        readStrings(v.get("team"), p.loadout.team);
        readStrings(v.get("skills"), p.loadout.skills);

        JsonValue codex = v.get("codex");
        if (codex != null) {
            for (JsonValue c = codex.child; c != null; c = c.next) {
                CodexEntry e = new CodexEntry();
                e.speciesId = c.getString("speciesId");
                e.caughtCount = c.getInt("caughtCount", 0);
                e.heaviest = c.getFloat("heaviest", 0f);
                e.firstCaughtAt = c.getLong("firstCaughtAt", 0L);
                e.lastCaughtAt = c.getLong("lastCaughtAt", 0L);
                p.codex.put(e.speciesId, e);
            }
        }
        return p;
    }

    private static AutoStrategy parseStrategy(String name) {
        for (AutoStrategy s : AutoStrategy.values()) {
            if (s.name().equals(name)) return s;
        }
        return AutoStrategy.BALANCED;
    }

    private static void readStrings(JsonValue array, java.util.List<String> out) {
        if (array == null) return;
        for (JsonValue v = array.child; v != null; v = v.next) {
            out.add(v.asString());
        }
    }

    private static String stringArray(java.util.List<String> values) {
        StringBuilder sb = new StringBuilder("[");
        for (int i = 0; i < values.size(); i++) {
            if (i > 0) sb.append(',');
            sb.append(quote(values.get(i)));
        }
        return sb.append(']').toString();
    }

    /** Minimal JSON string escaping; content ids and player ids are the only strings written. */
    private static String quote(String s) {
        if (s == null) return "null";
        StringBuilder sb = new StringBuilder(s.length() + 2).append('"');
        for (int i = 0; i < s.length(); i++) {
            char c = s.charAt(i);
            switch (c) {
                case '"': sb.append("\\\""); break;
                case '\\': sb.append("\\\\"); break;
                case '\n': sb.append("\\n"); break;
                case '\r': sb.append("\\r"); break;
                case '\t': sb.append("\\t"); break;
                default:
                    if (c < 0x20) {
                        sb.append("\\u").append(pad4(Integer.toHexString(c)));
                    } else {
                        sb.append(c);
                    }
            }
        }
        return sb.append('"').toString();
    }

    private static String pad4(String hex) {
        StringBuilder sb = new StringBuilder();
        for (int i = hex.length(); i < 4; i++) sb.append('0');
        return sb.append(hex).toString();
    }

    /** Unused today but kept beside the writer so the two stay in sync when rarity is persisted. */
    static Rarity parseRarity(String name) {
        return Rarity.fromId(name);
    }
}
