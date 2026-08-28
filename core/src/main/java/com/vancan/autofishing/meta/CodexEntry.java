package com.vancan.autofishing.meta;

/** Per-species collection progress (GDD 11). */
public final class CodexEntry {
    public String speciesId;
    public int caughtCount;
    public float heaviest;
    public long firstCaughtAt;
    public long lastCaughtAt;

    public boolean isDiscovered() {
        return caughtCount > 0;
    }
}
