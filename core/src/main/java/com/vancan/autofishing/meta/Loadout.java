package com.vancan.autofishing.meta;

import com.vancan.autofishing.content.GearSlot;

import java.util.ArrayList;
import java.util.EnumMap;
import java.util.List;
import java.util.Map;

/** What the player currently has equipped: gear per slot, the team, and the active skills. */
public final class Loadout {

    /** Slot -> id of the owned gear instance equipped there. */
    public final Map<GearSlot, String> equipped = new EnumMap<GearSlot, String>(GearSlot.class);

    /** Owned-angler ids. Index 0 is the captain and gets the captain bonus. */
    public final List<String> team = new ArrayList<String>();

    /** Skill ids taken into a fight, in HUD order. Capped by {@link #MAX_SKILLS}. */
    public final List<String> skills = new ArrayList<String>();

    public static final int MAX_TEAM = 5;
    public static final int MAX_SKILLS = 3;
    /** GDD 8.1: the captain contributes more, which is what makes formation a real choice. */
    public static final float CAPTAIN_BONUS = 1.5f;

    public String captain() {
        return team.isEmpty() ? null : team.get(0);
    }
}
