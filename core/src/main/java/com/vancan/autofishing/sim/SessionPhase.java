package com.vancan.autofishing.sim;

/** Session-level state machine from GDD 4.1. */
public enum SessionPhase {
    /** Waiting for a bite. */
    SEARCHING,
    /** A fish is on the bait; the hook has to be set inside the window. */
    BITE,
    /** The fight proper. */
    FIGHT,
    /** Terminal: fish landed. */
    LANDED,
    /** Terminal: lost, see {@link FishingSession#getFailure()}. */
    FAILED
}
