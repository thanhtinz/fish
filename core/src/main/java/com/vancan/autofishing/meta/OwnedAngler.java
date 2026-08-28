package com.vancan.autofishing.meta;

/** An angler a player has recruited (GDD 8). */
public final class OwnedAngler {
    public String id;
    public String defId;
    public int level = 1;
    public int stars = 1;

    public OwnedAngler() {
    }

    public OwnedAngler(String id, String defId, int level, int stars) {
        this.id = id;
        this.defId = defId;
        this.level = level;
        this.stars = stars;
    }
}
