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

    private static final float ROW_HEIGHT = 285f;
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

        // The silhouette is shown for undiscovered species too, in near-black. Revealing the
        // shape but not the identity is what makes the codex a hunt rather than a checklist -
        // a player can see there is a ray-shaped thing out there they have never landed.
        float iw = 230f, ih = iw * 0.5f;
        float ix = x + w - iw - 20f;
        float iy = y + (ROW_HEIGHT - ih) * 0.5f;
        if (found) {
            game.batch.setColor(Theme.rarityColor(species.rarity));
        } else {
            game.batch.setColor(0.22f, 0.27f, 0.33f, 1f);
        }
        game.batch.draw(art.fish(species.archetype), ix, iy, iw, ih);
        game.batch.setColor(com.badlogic.gdx.graphics.Color.WHITE);

        // Cursor stepped by the real line height. Fixed offsets put the stats line and the
        // description on the same y.
        float textW = ix - x - 44f;
        float small = art.fontSmall.getLineHeight();
        float line = y + ROW_HEIGHT - 22f;

        if (!found) {
            ui.text(art.font, "? ? ?", x + 24f, line, Theme.TEXT_DIM);
            line -= art.font.getLineHeight() + 6f;
            ui.text(art.fontSmall, species.rarity.displayName + "  ·  chưa bắt được",
                    x + 24f, line, Theme.TEXT_DIM);
            line -= small + 6f;
            ui.text(art.fontSmall, species.archetype.displayName, x + 24f, line, Theme.TEXT_DIM);
            return;
        }

        ui.text(art.font, species.name, x + 24f, line, Theme.rarityColor(species.rarity));
        line -= art.font.getLineHeight() + 6f;
        ui.text(art.fontSmall,
                species.rarity.displayName + "  ·  " + species.archetype.displayName,
                x + 24f, line, Theme.TEXT_DIM);
        line -= small + 6f;
        ui.text(art.fontSmall, "Bắt " + entry.caughtCount + " lần  ·  nặng nhất "
                        + Ui.weight(entry.heaviest), x + 24f, line, Theme.GOLD);
        line -= small + 6f;
        ui.textWrapped(art.fontSmall, species.description, x + 24f, line,
                textW, Theme.TEXT_DIM);
    }
}
