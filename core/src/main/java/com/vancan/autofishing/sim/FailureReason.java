package com.vancan.autofishing.sim;

/** Why a session ended without a fish. Drives both UI copy and the frustration telemetry (GDD 24). */
public enum FailureReason {
    NONE("—"),
    LINE_SNAPPED("Đứt dây"),
    SPOOLED("Cá kéo hết dây"),
    MISSED_HOOK("Trượt mồi"),
    FISH_ESCAPED("Cá sổng"),
    TIMEOUT("Hết giờ"),
    NO_BITE("Không có cá cắn"),
    RETREAT("Chủ động bỏ");

    public final String displayName;

    FailureReason(String displayName) {
        this.displayName = displayName;
    }
}
