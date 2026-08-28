package com.vancan.autofishing.screen;

import com.vancan.autofishing.VanCanGame;
import com.vancan.autofishing.content.SpotDef;
import com.vancan.autofishing.meta.PlayerFactory;
import com.vancan.autofishing.sim.EncounterTable;
import com.vancan.autofishing.sim.SpeciesDef;
import com.vancan.autofishing.ui.Theme;

/** Zone selection (GDD 10). Each row previews the pool so the choice is informed. */
public class MapScreen extends ScrollScreen {

    private static final float ROW_HEIGHT = 300f;

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
            SpotDef spot = spots.get(i);
            drawSpotRow(spot, x, y, w);
            y -= ROW_HEIGHT + Theme.PAD;
        }
    }

    private void drawSpotRow(SpotDef spot, float x, float y, float w) {
        boolean unlocked = PlayerFactory.isUnlocked(game.player, spot);
        boolean current = spot.id.equals(game.player.currentSpotId);

        ui.rect(x, y, w, ROW_HEIGHT, unlocked ? Theme.PANEL : Theme.BUTTON_DISABLED);
        ui.border(x, y, w, ROW_HEIGHT, 3f, current ? Theme.ACCENT : Theme.BORDER);

        ui.text(art.font, spot.name, x + 24f, y + ROW_HEIGHT - 34f,
                unlocked ? Theme.TEXT : Theme.TEXT_DIM);
        ui.textRight(art.fontSmall, "Bậc " + spot.tier, x + w - 24f, y + ROW_HEIGHT - 34f,
                Theme.TEXT_DIM);
        ui.text(art.fontSmall, spot.theme, x + 24f, y + ROW_HEIGHT - 78f, Theme.TEXT_DIM);

        // Pool preview: the rarest few entries are what a player actually chooses a zone for.
        StringBuilder pool = new StringBuilder();
        EncounterTable table = spot.buildTable(game.content.species);
        SpeciesDef rarest = null;
        for (EncounterTable.Entry e : table.getEntries()) {
            if (rarest == null || e.species.rarity.ordinal() > rarest.rarity.ordinal()) {
                rarest = e.species;
            }
        }
        int shown = 0;
        for (EncounterTable.Entry e : table.getEntries()) {
            if (shown++ >= 3) break;
            if (pool.length() > 0) pool.append(" · ");
            pool.append(e.species.name);
        }
        if (table.getEntries().size() > 3) pool.append(" · ...");
        ui.text(art.fontSmall, pool.toString(), x + 24f, y + ROW_HEIGHT - 122f, Theme.TEXT_DIM);

        if (rarest != null) {
            ui.text(art.fontSmall, "Hiếm nhất: " + rarest.name + " (" + rarest.rarity.displayName + ")",
                    x + 24f, y + ROW_HEIGHT - 164f, Theme.rarityColor(rarest.rarity));
        }
        if (spot.bossSpecies != null) {
            SpeciesDef boss = game.content.species.get(spot.bossSpecies);
            if (boss != null) {
                ui.textRight(art.fontSmall, "Boss: " + boss.name, x + w - 24f,
                        y + ROW_HEIGHT - 164f, Theme.WARN);
            }
        }

        float buttonY = y + 24f;
        float buttonH = Theme.TOUCH_MIN;
        if (!unlocked) {
            ui.text(art.fontSmall, "Mở khoá ở cấp " + spot.unlockLevel,
                    x + 24f, buttonY + buttonH / 2f + 8f, Theme.WARN);
        } else if (current) {
            ui.text(art.fontSmall, "Đang câu tại đây", x + 24f,
                    buttonY + buttonH / 2f + 8f, Theme.ACCENT);
        } else if (ui.button(x + 24f, buttonY, w - 48f, buttonH, "Đến ngư trường này")) {
            game.player.currentSpotId = spot.id;
            game.saveNow();
            game.setScreen(new FishingScreen(game));
        }

        if (unlocked) {
            ui.textWrapped(art.fontSmall, spot.description, x + 24f,
                    buttonY + buttonH + 52f, w - 48f, Theme.TEXT_DIM);
        }
    }
}
