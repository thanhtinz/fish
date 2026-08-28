package com.vancan.autofishing.sim;

/** An active skill usable during a fight (GDD 8). Authored in {@code assets/data/skills.json}. */
public final class SkillDef {

    public enum Effect {
        /** Multiplies damage for the duration. */
        BURST,
        /** Subtracts flat force from the tension target: the panic button. */
        BRACE,
        /** Restores line integrity. Rare and expensive. */
        MEND,
        /** Multiplies reel speed, closing distance fast. */
        WINCH,
        /** Drains fish stamina directly, opening the TIRED window early. */
        EXHAUST
    }

    public String id;
    public String name;
    public Effect effect = Effect.BURST;
    public float magnitude = 1.5f;
    public float durationSeconds = 3f;
    public float cooldownSeconds = 12f;
    public String description = "";
}
