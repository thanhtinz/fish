package com.vancan.autofishing.meta;

/** An instance of a gear template in a player's inventory. */
public final class OwnedGear {
    public String id;
    public String templateId;
    public int level = 1;

    public OwnedGear() {
    }

    public OwnedGear(String id, String templateId, int level) {
        this.id = id;
        this.templateId = templateId;
        this.level = level;
    }
}
