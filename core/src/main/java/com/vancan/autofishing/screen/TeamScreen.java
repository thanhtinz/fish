package com.vancan.autofishing.screen;

import com.vancan.autofishing.VanCanGame;
import com.vancan.autofishing.content.AnglerDef;
import com.vancan.autofishing.meta.Loadout;
import com.vancan.autofishing.meta.OwnedAngler;
import com.vancan.autofishing.meta.PlayerState;
import com.vancan.autofishing.ui.Theme;

/** Team formation and talents (GDD 8). The captain slot is what makes ordering matter. */
public class TeamScreen extends ScrollScreen {

    private static final float ROW_HEIGHT = 310f;

    public TeamScreen(VanCanGame game) {
        super(game);
    }

    @Override
    protected String title() {
        return "Đội cần thủ";
    }

    @Override
    protected int navIndex() {
        return 3;
    }

    @Override
    protected void drawRows(float delta) {
        PlayerState p = game.player;
        float talentHeight = 360f;
        setContentLength(talentHeight + Theme.PAD
                + p.anglers.size() * (ROW_HEIGHT + Theme.PAD) + 120f);

        float x = Theme.PAD;
        float w = Theme.WORLD_WIDTH - Theme.PAD * 2f;
        float y = cursorY() - talentHeight;

        drawTalents(x, y, w, talentHeight);
        y -= Theme.PAD + 60f;

        // Each line has to clear the one above it by a full line height: font.draw takes the
        // top of the line, not the baseline, so fixed gaps overlap at this font size.
        ui.text(art.font, "Đội hình  (" + p.loadout.team.size() + "/" + Loadout.MAX_TEAM + ")",
                x, y + 40f, Theme.ACCENT);
        ui.text(art.fontSmall, "Người đầu tiên là đội trưởng, đóng góp nhiều hơn.",
                x, y + 40f - art.font.getLineHeight() - 8f, Theme.TEXT_DIM);
        y -= art.font.getLineHeight() + art.fontSmall.getLineHeight() + 20f;

        for (OwnedAngler owned : p.anglers.values()) {
            y -= ROW_HEIGHT;
            drawAnglerRow(owned, x, y, w);
            y -= Theme.PAD;
        }
    }

    private void drawTalents(float x, float y, float w, float h) {
        PlayerState p = game.player;
        ui.panel(x, y, w, h);
        ui.text(art.font, "Tiềm năng", x + 24f, y + h - 34f, Theme.ACCENT);
        ui.textRight(art.fontSmall, "Điểm chưa dùng: " + p.talentPoints,
                x + w - 24f, y + h - 34f, p.talentPoints > 0 ? Theme.GOLD : Theme.TEXT_DIM);

        float rowY = y + h - 120f;
        rowY = talentRow("Lực kéo", p.talentPull, x, rowY, w, 0);
        rowY = talentRow("An toàn", p.talentSafety, x, rowY, w, 1);
        rowY = talentRow("May mắn", p.talentLuck, x, rowY, w, 2);
        talentRow("Ngoại tuyến", p.talentOffline, x, rowY, w, 3);
    }

    private float talentRow(String label, int value, float x, float y, float w, int index) {
        ui.text(art.fontSmall, label, x + 24f, y + 30f, Theme.TEXT);
        ui.textRight(art.fontSmall, "+" + value, x + w - 140f, y + 30f, Theme.TEXT_DIM);
        boolean canSpend = game.player.talentPoints > 0;
        if (ui.button(x + w - 110f, y, 86f, 64f, "+", canSpend, false)) {
            spendTalent(index);
        }
        return y - 74f;
    }

    private void spendTalent(int index) {
        PlayerState p = game.player;
        if (p.talentPoints <= 0) return;
        p.talentPoints--;
        switch (index) {
            case 0: p.talentPull++; break;
            case 1: p.talentSafety++; break;
            case 2: p.talentLuck++; break;
            default: p.talentOffline++; break;
        }
        game.saveNow();
    }

    private void drawAnglerRow(OwnedAngler owned, float x, float y, float w) {
        AnglerDef def = game.content.anglers.get(owned.defId);
        if (def == null) return;

        PlayerState p = game.player;
        int teamIndex = p.loadout.team.indexOf(owned.id);
        boolean inTeam = teamIndex >= 0;
        boolean isCaptain = teamIndex == 0;

        ui.panel(x, y, w, ROW_HEIGHT);
        if (isCaptain) ui.border(x, y, w, ROW_HEIGHT, 3f, Theme.GOLD);

        // Portrait on the left, text column beside it. A rarity ring rather than a coloured
        // name alone: at a glance the roster should sort by strength without reading it.
        float portraitSize = 150f;
        float px = x + 24f;
        float py = y + ROW_HEIGHT - portraitSize - 24f;
        game.batch.setColor(Theme.rarityColor(def.rarity));
        game.batch.draw(art.circle, px - 6f, py - 6f, portraitSize + 12f, portraitSize + 12f);
        game.batch.setColor(com.badlogic.gdx.graphics.Color.WHITE);
        game.batch.draw(art.portrait(def.id), px, py, portraitSize, portraitSize);

        float textX = px + portraitSize + 22f;
        float line = y + ROW_HEIGHT - 22f;
        ui.text(art.font, def.name, textX, line, Theme.rarityColor(def.rarity));
        ui.textRight(art.fontSmall,
                def.role.displayName + "  ·  Lv" + owned.level + "  ·  " + owned.stars + "★",
                x + w - 24f, line, Theme.TEXT);

        line -= art.font.getLineHeight() + 6f;
        ui.text(art.fontSmall, def.role.summary, textX, line, Theme.ACCENT);

        line -= art.fontSmall.getLineHeight() + 6f;
        ui.textWrapped(art.fontSmall, def.description, textX, line,
                w - (textX - x) - 24f, Theme.TEXT_DIM);

        float buttonH = Theme.TOUCH_MIN;
        float buttonY = y + 22f;
        float half = (w - 72f) / 2f;

        if (inTeam) {
            if (ui.button(x + 24f, buttonY, half, buttonH, "Rời đội")) {
                p.loadout.team.remove(owned.id);
                game.saveNow();
            }
            boolean canPromote = !isCaptain;
            if (ui.button(x + 48f + half, buttonY, half, buttonH,
                    isCaptain ? "Đội trưởng" : "Làm đội trưởng", canPromote, isCaptain)) {
                p.loadout.team.remove(owned.id);
                p.loadout.team.add(0, owned.id);
                game.saveNow();
            }
        } else {
            boolean hasRoom = p.loadout.team.size() < Loadout.MAX_TEAM;
            if (ui.button(x + 24f, buttonY, w - 48f, buttonH,
                    hasRoom ? "Vào đội" : "Đội đã đầy", hasRoom, false)) {
                p.loadout.team.add(owned.id);
                game.saveNow();
            }
        }
    }
}
