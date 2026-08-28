package com.vancan.autofishing.meta;

import com.vancan.autofishing.content.AnglerDef;
import com.vancan.autofishing.content.GameContent;
import com.vancan.autofishing.content.GearSlot;
import com.vancan.autofishing.content.GearTemplate;
import com.vancan.autofishing.sim.BuildStats;
import com.vancan.autofishing.sim.SkillDef;

import java.util.ArrayList;
import java.util.List;

/**
 * Flattens a player's inventory into the {@link BuildStats} the simulation consumes.
 *
 * <p>This is the single seam between the meta game and the simulation. Keeping it in one place is
 * what lets the server recompute a player's true build during settlement and compare it against
 * what the client claimed (GDD 19).
 */
public final class LoadoutResolver {

    private LoadoutResolver() {
    }

    public static BuildStats resolve(PlayerState player, GameContent content) {
        // Start from a zeroed base rather than the BuildStats defaults: with defaults, an unequipped
        // slot would silently contribute starter-gear stats and the numbers would not add up.
        BuildStats b = zeroed();

        for (GearSlot slot : GearSlot.values()) {
            String ownedId = player.loadout.equipped.get(slot);
            if (ownedId == null) continue;
            OwnedGear owned = player.gear.get(ownedId);
            if (owned == null) continue;
            GearTemplate template = content.gear.get(owned.templateId);
            if (template == null) {
                throw new IllegalStateException("Owned gear '" + ownedId
                        + "' references unknown template '" + owned.templateId + "'");
            }
            template.apply(b, owned.level);
        }

        for (int i = 0; i < player.loadout.team.size(); i++) {
            OwnedAngler owned = player.anglers.get(player.loadout.team.get(i));
            if (owned == null) continue;
            AnglerDef def = content.anglers.get(owned.defId);
            if (def == null) continue;
            float captainBonus = (i == 0) ? Loadout.CAPTAIN_BONUS : 1f;
            def.apply(b, owned.level, owned.stars, captainBonus);
        }

        applyTalents(b, player);
        clampToSaneRanges(b);
        return b;
    }

    /** Talent tree contributions (GDD 8): flat, readable, and capped by talent points spent. */
    private static void applyTalents(BuildStats b, PlayerState player) {
        b.rodPower += player.talentPull * 1.6f;
        b.safetyMitigation += player.talentSafety * 1.2f;
        b.luck += player.talentLuck * 0.02f;
        b.offlineEfficiency += player.talentOffline * 0.04f;
    }

    /**
     * Probabilities must not exceed 1 no matter how much a player stacks, and the simulation
     * divides by several of these, so a zero would produce infinities rather than a bad-but-valid
     * fight.
     */
    private static void clampToSaneRanges(BuildStats b) {
        b.critChance = clamp(b.critChance, 0f, 0.85f);
        b.hookRate = clamp(b.hookRate, 0.05f, 0.99f);
        b.critHook = clamp(b.critHook, 0f, 0.5f);
        b.control = Math.max(0.4f, b.control);
        b.elasticity = Math.max(0f, b.elasticity);
        b.recovery = Math.max(0f, b.recovery);
        b.attraction = Math.max(0.1f, b.attraction);
        b.biteDetection = Math.max(0.1f, b.biteDetection);
        b.weightModifier = Math.max(0.1f, b.weightModifier);
        b.luck = Math.max(1f, b.luck);
        b.distanceControl = clamp(b.distanceControl, 0f, 0.75f);
        b.offlineEfficiency = Math.max(0.1f, b.offlineEfficiency);
        b.lineLength = Math.max(5f, b.lineLength);
        b.lineStrength = Math.max(1f, b.lineStrength);
        b.pullSpeed = Math.max(0.2f, b.pullSpeed);
    }

    private static float clamp(float v, float lo, float hi) {
        return v < lo ? lo : (v > hi ? hi : v);
    }

    /** A build with every additive stat at its neutral value. */
    public static BuildStats zeroed() {
        BuildStats b = new BuildStats();
        b.rodPower = 0; b.control = 1f; b.critChance = 0f;
        b.reelPower = 0; b.pullSpeed = 0; b.maxDrag = 0; b.recovery = 1f;
        b.lineStrength = 0; b.lineLength = 0; b.elasticity = 1f;
        b.hookRate = 0; b.critHook = 0;
        b.biteDetection = 1f; b.rareDetection = 0;
        b.attraction = 1f; b.rarityBias = 0; b.weightModifier = 1f; b.speciesBias = null;
        b.teamPull = 0; b.safetyMitigation = 0; b.damageBonus = 1f;
        b.distanceControl = 0; b.luck = 1f; b.offlineEfficiency = 1f;
        return b;
    }

    /** Resolves the equipped skill definitions in HUD order, skipping any that are unknown. */
    public static SkillDef[] resolveSkills(PlayerState player, GameContent content) {
        List<SkillDef> out = new ArrayList<SkillDef>();
        for (int i = 0; i < player.loadout.skills.size() && i < Loadout.MAX_SKILLS; i++) {
            SkillDef def = content.skills.get(player.loadout.skills.get(i));
            if (def != null) out.add(def);
        }
        return out.toArray(new SkillDef[0]);
    }
}
