package com.vancan.autofishing.screen;

import com.vancan.autofishing.VanCanGame;
import com.vancan.autofishing.content.GearSlot;
import com.vancan.autofishing.content.GearTemplate;
import com.vancan.autofishing.meta.Currency;
import com.vancan.autofishing.meta.LoadoutResolver;
import com.vancan.autofishing.meta.OwnedGear;
import com.vancan.autofishing.meta.PlayerState;
import com.vancan.autofishing.sim.BuildStats;
import com.vancan.autofishing.ui.Theme;
import com.vancan.autofishing.ui.Ui;

/**
 * Gear management (GDD 9): the equipped item per slot, what upgrading it costs, and the resolved
 * build totals so a player can see what a change actually did to their numbers.
 */
public class GearScreen extends ScrollScreen {

    private static final float ROW_HEIGHT = 250f;
    private String message;
    private float messageTimer;

    public GearScreen(VanCanGame game) {
        super(game);
    }

    @Override
    protected String title() {
        return "Ngư cụ";
    }

    @Override
    protected int navIndex() {
        return 2;
    }

    @Override
    protected void update(float delta) {
        super.update(delta);
        if (messageTimer > 0f) messageTimer -= delta;
    }

    @Override
    protected void drawRows(float delta) {
        GearSlot[] slots = GearSlot.values();
        float summaryHeight = 320f;
        setContentLength(summaryHeight + slots.length * (ROW_HEIGHT + Theme.PAD));

        float x = Theme.PAD;
        float w = Theme.WORLD_WIDTH - Theme.PAD * 2f;
        float y = cursorY() - summaryHeight;

        drawBuildSummary(x, y, w, summaryHeight);
        y -= Theme.PAD;

        for (GearSlot slot : slots) {
            y -= ROW_HEIGHT;
            drawSlotRow(slot, x, y, w);
            y -= Theme.PAD;
        }
    }

    private void drawBuildSummary(float x, float y, float w, float h) {
        BuildStats b = LoadoutResolver.resolve(game.player, game.content);
        ui.panel(x, y, w, h);
        ui.text(art.font, "Tổng chỉ số", x + 24f, y + h - 34f, Theme.ACCENT);

        float col = w / 2f;
        float ry = y + h - 96f;
        stat("Lực kéo", Ui.trim1(b.rodPower + b.reelPower + b.teamPull), x + 24f, ry, col - 40f);
        stat("Sức bền dây", Math.round(b.breakingTension()) + "", x + col, ry, col - 40f);
        ry -= 56f;
        stat("Dài dây", Math.round(b.lineLength) + " m", x + 24f, ry, col - 40f);
        stat("Tốc độ thu", Ui.trim1(b.pullSpeed) + " m/s", x + col, ry, col - 40f);
        ry -= 56f;
        stat("Tỉ lệ dính", Ui.percent(b.hookRate), x + 24f, ry, col - 40f);
        stat("Chí mạng", Ui.percent(b.critChance), x + col, ry, col - 40f);
        ry -= 56f;
        stat("May mắn", Ui.trim1(b.luck) + "x", x + 24f, ry, col - 40f);
        stat("Giảm căng", Ui.trim1(b.safetyMitigation), x + col, ry, col - 40f);

        if (messageTimer > 0f && message != null) {
            ui.textCentered(art.fontSmall, message, Theme.WORLD_WIDTH / 2f, y + 20f, Theme.GOLD);
        }
    }

    private void stat(String label, String value, float x, float y, float w) {
        ui.text(art.fontSmall, label, x, y, Theme.TEXT_DIM);
        ui.textRight(art.fontSmall, value, x + w, y, Theme.TEXT);
    }

    private void drawSlotRow(GearSlot slot, float x, float y, float w) {
        PlayerState p = game.player;
        String ownedId = p.loadout.equipped.get(slot);
        OwnedGear owned = ownedId == null ? null : p.gear.get(ownedId);
        GearTemplate template = owned == null ? null : game.content.gear.get(owned.templateId);

        ui.panel(x, y, w, ROW_HEIGHT);
        ui.text(art.fontSmall, slot.displayName, x + 24f, y + ROW_HEIGHT - 30f, Theme.TEXT_DIM);

        if (template == null) {
            ui.text(art.font, "— trống —", x + 24f, y + ROW_HEIGHT - 84f, Theme.TEXT_DIM);
            return;
        }

        ui.text(art.font, template.name, x + 24f, y + ROW_HEIGHT - 84f,
                Theme.rarityColor(template.rarity));
        ui.textRight(art.fontSmall, "Cấp " + owned.level + "  ·  Bậc " + template.tier,
                x + w - 24f, y + ROW_HEIGHT - 84f, Theme.TEXT);
        ui.text(art.fontSmall, template.description, x + 24f, y + ROW_HEIGHT - 128f,
                Theme.TEXT_DIM);

        float buttonH = Theme.TOUCH_MIN;
        float buttonY = y + 22f;
        float half = (w - 72f) / 2f;

        int cost = template.upgradeCost(owned.level);
        boolean affordable = p.canAfford(Currency.GOLD, cost);
        if (ui.button(x + 24f, buttonY, half, buttonH,
                "Nâng cấp  " + Ui.number(cost) + " ⧫", affordable, false)) {
            upgrade(owned, template, cost);
        }

        GearTemplate next = nextTier(slot, template.tier);
        if (next == null) {
            ui.text(art.fontSmall, "Đã tốt nhất", x + 48f + half, buttonY + buttonH / 2f + 8f,
                    Theme.TEXT_DIM);
        } else {
            int buyCost = buyCost(next);
            boolean owns = ownsTemplate(next.id);
            boolean canBuy = owns || p.canAfford(Currency.GOLD, buyCost);
            String label = owns ? "Trang bị " + next.name
                    : "Mua " + next.name + "  " + Ui.number(buyCost) + " ⧫";
            if (ui.button(x + 48f + half, buttonY, half, buttonH, label, canBuy, false)) {
                buyOrEquip(slot, next, buyCost, owns);
            }
        }
    }

    private void upgrade(OwnedGear owned, GearTemplate template, int cost) {
        if (game.player.applyCurrency(Currency.GOLD, -cost, "upgrade:" + template.id,
                "upg:" + owned.id + ":" + owned.level)) {
            owned.level++;
            note(template.name + " lên cấp " + owned.level);
            game.saveNow();
        }
    }

    private void buyOrEquip(GearSlot slot, GearTemplate next, int cost, boolean owns) {
        String ownedId = "own_" + next.id;
        if (!owns) {
            if (!game.player.applyCurrency(Currency.GOLD, -cost, "buy:" + next.id,
                    "buy:" + next.id)) {
                return;
            }
            game.player.gear.put(ownedId, new OwnedGear(ownedId, next.id, 1));
            note("Đã mua " + next.name);
        } else {
            note("Đã trang bị " + next.name);
        }
        game.player.loadout.equipped.put(slot, ownedId);
        game.saveNow();
    }

    private boolean ownsTemplate(String templateId) {
        for (OwnedGear g : game.player.gear.values()) {
            if (g.templateId.equals(templateId)) return true;
        }
        return false;
    }

    /** Shop price for the next item in a slot; scales with tier so it stays a real sink. */
    private int buyCost(GearTemplate template) {
        return (int) (320 * Math.pow(2.35, template.tier - 1));
    }

    private GearTemplate nextTier(GearSlot slot, int currentTier) {
        GearTemplate best = null;
        for (GearTemplate g : game.content.gearForSlot(slot)) {
            if (g.tier <= currentTier) continue;
            if (best == null || g.tier < best.tier) best = g;
        }
        return best;
    }

    private void note(String text) {
        message = text;
        messageTimer = 2.5f;
    }
}
