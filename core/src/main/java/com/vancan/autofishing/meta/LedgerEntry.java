package com.vancan.autofishing.meta;

/**
 * One economy mutation (GDD 20 EconomyLedger, GDD 19 audit log).
 *
 * <p>Every currency change goes through the ledger rather than mutating a balance directly, so
 * inflation can be traced to a source and a duplicated reward is visible after the fact instead of
 * only as a suspiciously rich player.
 */
public final class LedgerEntry {
    public long at;
    public Currency currency;
    /** Positive for a source, negative for a sink. */
    public int amount;
    /** Where it came from: "catch:ca_chep", "upgrade:rod_t2", "offline:settle". */
    public String reason;
    /** Idempotency key; a repeated key must not be applied twice. */
    public String txId;
}
