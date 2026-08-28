package com.vancan.autofishing.meta;

/** GDD 11.3. */
public enum Currency {
    GOLD("Vàng"), GEMS("Ngọc"), TICKETS("Vé"), ESSENCE("Tinh Phách");

    public final String displayName;

    Currency(String displayName) {
        this.displayName = displayName;
    }
}
