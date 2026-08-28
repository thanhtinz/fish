package com.vancan.autofishing.screen;

import com.vancan.autofishing.VanCanGame;
import com.vancan.autofishing.meta.CodexEntry;
import com.vancan.autofishing.sim.Rarity;
import com.vancan.autofishing.sim.SpeciesDef;
import com.vancan.autofishing.ui.Theme;
import com.vancan.autofishing.ui.Ui;

/**
 * The fish codex (GDD 11). Undiscovered species are listed but redacted: showing that something
 * exists without showing what gives the collection a target to chase.
 */
public class CodexScreen extends ScrollScreen {

    private static final float ROW_HEIGHT = 165f;
    private Rarity filter;

    public CodexScreen(VanCanGame game) {
        super(game);
    }

    @Override
    protected String title() {
        return "Đồ giám";
    }

    @Override
    protected int navIndex() {
        return 4;
    }

    @Override
    protected void drawRows(float delta) {
        float headerHeight = 260f;
        java.util.List<SpeciesDef> shown = new java.util.ArrayList<SpeciesDef>();
        for (SpeciesDef s : game.content.species.values()) {
            if (filter == null || s.rarity == filter) shown.add(s);
        }
        setContentLength(headerHeight + shown.size() * (ROW_HEIGHT + 12f));

        float x = Theme.PAD;
        float w = Theme.WORLD_WIDTH - Theme.PAD * 2f;
        float y = cursorY() - headerHeight;

        drawSummary(x, y, w, headerHeight);
        y -= 12f;

        for (SpeciesDef species : shown) {
            y -= ROW_HEIGHT;
            drawSpeciesRow(species, x, y, w);
            y -= 12f;
        }
    }

    private void drawSummary(float x, float y, float w, float h) {
        int total = game.content.species.size();
        int found = game.player.discoveredSpecies();

        ui.panel(x, y, w, h);
        // Text descends from the y given to font.draw, so the bar has to clear a full line
        // height below the title rather than a fixed gap.
        float titleTop = y + h - 22f;
        ui.text(art.font, "Đã ghi nhận " + found + " / " + total, x + 24f, titleTop, Theme.ACCENT);
        ui.bar(x + 24f, titleTop - art.font.getLineHeight() - 14f, w - 48f, 18f,
                total == 0 ? 0f : found / (float) total, Theme.ACCENT, Theme.PANEL_LIGHT);

        // Rarity filter chips.
        float chipW = (w - 48f) / 4f - 8f;
        float chipY = y + 24f;
        if (ui.button(x + 24f, chipY, chipW, 64f, "Tất cả", true, filter == null)) filter = null;
        Rarity[] quick = {Rarity.RARE, Rarity.LEGENDARY, Rarity.MYTHIC};
        for (int i = 0; i < quick.length; i++) {
            float cx = x + 24f + (i + 1) * (chipW + 10f);
            if (ui.button(cx, chipY, chipW, 64f, quick[i].displayName, true, filter == quick[i])) {
                filter = filter == quick[i] ? null : quick[i];
            }
        }
    }

    private void drawSpeciesRow(SpeciesDef species, float x, float y, float w) {
        CodexEntry entry = game.player.codex.get(species.id);
        boolean found = entry != null && entry.isDiscovered();

        ui.rect(x, y, w, ROW_HEIGHT, found ? Theme.PANEL : Theme.BUTTON_DISABLED);
        ui.border(x, y, w, ROW_HEIGHT, 2f, found ? Theme.rarityColor(species.rarity) : Theme.BORDER);

        if (!found) {
            ui.text(art.font, "? ? ?", x + 24f, y + ROW_HEIGHT - 42f, Theme.TEXT_DIM);
            ui.text(art.fontSmall, species.rarity.displayName + "  ·  chưa bắt được",
                    x + 24f, y + ROW_HEIGHT - 92f, Theme.TEXT_DIM);
            ui.textRight(art.fontSmall, species.archetype.displayName,
                    x + w - 24f, y + ROW_HEIGHT - 42f, Theme.TEXT_DIM);
            return;
        }

        ui.text(art.font, species.name, x + 24f, y + ROW_HEIGHT - 42f,
                Theme.rarityColor(species.rarity));
        ui.textRight(art.fontSmall, species.rarity.displayName, x + w - 24f,
                y + ROW_HEIGHT - 42f, Theme.rarityColor(species.rarity));

        ui.text(art.fontSmall,
                species.archetype.displayName + "  ·  bắt " + entry.caughtCount + " lần",
                x + 24f, y + ROW_HEIGHT - 88f, Theme.TEXT_DIM);
        ui.textRight(art.fontSmall, "Nặng nhất " + Ui.weight(entry.heaviest),
                x + w - 24f, y + ROW_HEIGHT - 88f, Theme.GOLD);
        ui.text(art.fontSmall, species.description, x + 24f, y + 34f, Theme.TEXT_DIM);
    }
}
