package com.vancan.autofishing.screen;

import com.vancan.autofishing.VanCanGame;
import com.vancan.autofishing.content.SpotDef;
import com.vancan.autofishing.meta.PlayerFactory;
import com.vancan.autofishing.sim.EncounterTable;
import com.vancan.autofishing.sim.SpeciesDef;
import com.vancan.autofishing.ui.Theme;

/** Zone selection (GDD 10). Each row previews the pool so the choice is informed. */
public class MapScreen extends ScrollScreen {

    private static final float ROW_HEIGHT = 430f;

    public MapScreen(VanCanGame game) {
        super(game);
    }

    @Override
    protected String title() {
        return "Bản đồ ngư trường";
    }

    @Override
    protected int navIndex() {
        return 1;
    }

    @Override
    protected void drawRows(float delta) {
        java.util.List<SpotDef> spots = game.content.spotsInOrder();
        setContentLength(spots.size() * (ROW_HEIGHT + Theme.PAD));

        float y = cursorY() - ROW_HEIGHT;
        float x = Theme.PAD;
        float w = Theme.WORLD_WIDTH - Theme.PAD * 2f;

        for (int i = 0; i < spots.size(); i++) {
            drawSpotRow(spots.get(i), x, y, w);
            y -= ROW_HEIGHT + Theme.PAD;
        }
    }

    /**
     * One zone card. Laid out with a downward cursor stepped by the real font line height:
     * fixed offsets overlapped badly here, because the pool preview and the description are both
     * variable-length and the boss label shared a line with the rarest-fish label.
     */
    private void drawSpotRow(SpotDef spot, float x, float y, float w) {
        boolean unlocked = PlayerFactory.isUnlocked(game.player, spot);
        boolean current = spot.id.equals(game.player.currentSpotId);

        ui.rect(x, y, w, ROW_HEIGHT, unlocked ? Theme.PANEL : Theme.BUTTON_DISABLED);
        ui.border(x, y, w, ROW_HEIGHT, 3f, current ? Theme.ACCENT : Theme.BORDER);

        float left = x + 24f;
        float inner = w - 48f;
        float big = art.font.getLineHeight();
        float small = art.fontSmall.getLineHeight();
        float line = y + ROW_HEIGHT - 22f;

        ui.text(art.font, spot.name, left, line, unlocked ? Theme.TEXT : Theme.TEXT_DIM);
        ui.textRight(art.fontSmall, "Bậc " + spot.tier, left + inner, line, Theme.TEXT_DIM);
        line -= big + 4f;

        ui.text(art.fontSmall, spot.theme, left, line, Theme.TEXT_DIM);
        line -= small + 4f;

        EncounterTable table = spot.buildTable(game.content.species);
        SpeciesDef rarest = null;
        StringBuilder pool = new StringBuilder();
        int shown = 0;
        for (EncounterTable.Entry e : table.getEntries()) {
            if (rarest == null || e.species.rarity.ordinal() > rarest.rarity.ordinal()) {
                rarest = e.species;
            }
            // Two names plus an ellipsis: three overflowed the width on the deep-sea zones,
            // whose species have the longest names.
            if (shown < 2) {
                if (shown > 0) pool.append("  ·  ");
                pool.append(e.species.name);
                shown++;
            }
        }
        if (table.getEntries().size() > shown) pool.append("  ·  ...");
        ui.text(art.fontSmall, pool.toString(), left, line, Theme.TEXT_DIM);
        line -= small + 4f;

        if (rarest != null) {
            ui.text(art.fontSmall, "Hiếm nhất: " + rarest.name + " (" + rarest.rarity.displayName + ")",
                    left, line, Theme.rarityColor(rarest.rarity));
            line -= small + 4f;
        }

        // Boss gets its own line; sharing one with the rarest-fish label collided on every zone
        // that has both.
        if (spot.bossSpecies != null) {
            SpeciesDef boss = game.content.species.get(spot.bossSpecies);
            if (boss != null) {
                ui.text(art.fontSmall, "Boss: " + boss.name, left, line, Theme.WARN);
                line -= small + 4f;
            }
        }

        ui.textWrapped(art.fontSmall, spot.description, left, line, inner, Theme.TEXT_DIM);

        float buttonH = Theme.TOUCH_MIN;
        float buttonY = y + 22f;
        if (!unlocked) {
            ui.rect(left, buttonY, inner, buttonH, Theme.BUTTON_DISABLED);
            ui.border(left, buttonY, inner, buttonH, 2f, Theme.BORDER);
            ui.textCentered(art.fontSmall, "Mở khoá ở cấp " + spot.unlockLevel,
                    x + w / 2f, buttonY + buttonH / 2f + 10f, Theme.WARN);
        } else if (current) {
            ui.rect(left, buttonY, inner, buttonH, Theme.BUTTON_ACTIVE, 0.30f);
            ui.border(left, buttonY, inner, buttonH, 2f, Theme.ACCENT);
            ui.textCentered(art.fontSmall, "Đang câu tại đây",
                    x + w / 2f, buttonY + buttonH / 2f + 10f, Theme.ACCENT);
        } else if (ui.button(left, buttonY, inner, buttonH, "Đến ngư trường này")) {
            game.player.currentSpotId = spot.id;
            game.saveNow();
            game.setScreen(new FishingScreen(game));
        }
    }
}
