package com.vancan.autofishing.screen;

import com.badlogic.gdx.graphics.Color;
import com.vancan.autofishing.VanCanGame;
import com.vancan.autofishing.auto.AutoStrategy;
import com.vancan.autofishing.meta.FishRecord;
import com.vancan.autofishing.meta.FishingController;
import com.vancan.autofishing.meta.OfflineSettlement;
import com.vancan.autofishing.sim.EncounterTable;
import com.vancan.autofishing.sim.FailureReason;
import com.vancan.autofishing.sim.FishState;
import com.vancan.autofishing.sim.FishingSession;
import com.vancan.autofishing.sim.SessionPhase;
import com.vancan.autofishing.sim.SkillRuntime;
import com.vancan.autofishing.ui.Theme;
import com.vancan.autofishing.ui.Ui;

/**
 * The main screen (GDD 16): the water simulation takes the upper two-thirds and the HUD shows only
 * the state a player needs to make a decision - tension, line integrity, distance, fish condition,
 * phase and the Auto policy.
 */
public class FishingScreen extends BaseScreen {

    private final FishingController controller;
    private float time;
    private boolean showStrategyPicker;
    private OfflineSettlement.Result offlineReport;

    /** Smoothed gauge values; the raw simulation numbers are too twitchy to read at 30Hz. */
    private float shownTension;
    private float shownDistance;

    public FishingScreen(VanCanGame game) {
        super(game);
        controller = new FishingController(game.content, game.player);
        offlineReport = game.pendingOfflineReport;
        game.pendingOfflineReport = null;
        if (game.player.autoEnabled) controller.cast();
    }

    @Override
    protected String title() {
        return controller.spot() == null ? "Câu cá" : controller.spot().name;
    }

    @Override
    protected int navIndex() {
        return 0;
    }

    @Override
    protected void update(float delta) {
        time += delta;
        // The offline panel is modal: pausing the loop behind it stops the player's first cast
        // resolving unseen while they are still reading their idle rewards.
        if (offlineReport == null) {
            controller.update(delta, VanCanGame.nowMillis());
        }

        FishingSession s = controller.session();
        float targetTension = s == null ? 0f : s.getTensionRatio();
        float targetDistance = s == null ? 0f : s.getDistanceRatio();
        float k = Math.min(1f, delta * 12f);
        shownTension += (targetTension - shownTension) * k;
        shownDistance += (targetDistance - shownDistance) * k;
    }

    // --- Vertical layout budget -------------------------------------------------------
    //
    // The portrait screen is divided once, here, rather than each block guessing its own
    // offsets. An earlier version computed positions inline and the gauge labels ended up
    // drawn on top of their own bars.
    private static final float WATER_BOTTOM = 985f;
    private static final float GAUGE_TOP = 975f;
    private static final float SKILL_Y = 505f;
    private static final float STRATEGY_ROW_Y = 355f;
    private static final float ACTION_ROW_Y = 190f;
    private static final float ROW_H = Theme.TOUCH_MIN;

    private float waterBottom() {
        return WATER_BOTTOM;
    }

    @Override
    protected void drawContent(float delta) {
        drawWater();
        drawFightVisual();
        drawGauges();
        drawControls();

        if (controller.isIdle()
                && (controller.lastCatch() != null
                    || controller.lastFailure() != FailureReason.NONE)) {
            drawResultBanner();
        }
        if (offlineReport != null) {
            drawOfflineReport();
        }
    }

    private void drawWater() {
        float top = contentTop();
        float bottom = waterBottom();
        float height = top - bottom;
        float surface = bottom + height * 0.86f;

        ui.rect(0, surface, Theme.WORLD_WIDTH, top - surface, Theme.SKY_BOTTOM);

        // Banded gradient from surface colour down to deep. Cheap, and it reads as depth.
        int bands = 24;
        float bandHeight = (surface - bottom) / bands;
        for (int i = 0; i < bands; i++) {
            float t = i / (float) (bands - 1);
            float r = Theme.WATER_TOP.r + (Theme.WATER_DEEP.r - Theme.WATER_TOP.r) * t;
            float g = Theme.WATER_TOP.g + (Theme.WATER_DEEP.g - Theme.WATER_TOP.g) * t;
            float b = Theme.WATER_TOP.b + (Theme.WATER_DEEP.b - Theme.WATER_TOP.b) * t;
            game.batch.setColor(r, g, b, 1f);
            game.batch.draw(art.pixel, 0, surface - (i + 1) * bandHeight,
                    Theme.WORLD_WIDTH, bandHeight + 1f);
        }
        game.batch.setColor(Color.WHITE);

        for (int i = 0; i < 26; i++) {
            float x = i * (Theme.WORLD_WIDTH / 25f);
            float wave = (float) Math.sin(time * 1.6f + i * 0.55f) * 7f;
            ui.rect(x - 24f, surface + wave, 48f, 5f, Theme.ACCENT, 0.35f);
        }

        drawBubbles(bottom, surface);
    }

    private void drawBubbles(float bottom, float surface) {
        // Deterministic pseudo-random placement from the index; no allocation, no state.
        for (int i = 0; i < 14; i++) {
            float seedX = (i * 97 % 100) / 100f;
            float speed = 26f + (i * 37 % 40);
            float span = surface - bottom;
            float phase = (time * speed + i * 120f) % span;
            float x = seedX * Theme.WORLD_WIDTH;
            float size = 10f + (i % 4) * 6f;
            float alpha = 0.20f * (1f - phase / span);
            game.batch.setColor(1f, 1f, 1f, alpha);
            game.batch.draw(art.bubble, x, bottom + phase, size, size);
        }
        game.batch.setColor(Color.WHITE);
    }

    // Scene composition: the angler stands in a boat on the left, the rod arcs out to the right,
    // and the fish works the open water under it. A centred angler with a vertical line left the
    // fish nowhere to swim and made distance impossible to read.
    private static final float ANGLER_X = 0.24f;
    private static final float ROD_LENGTH = 300f;
    /**
     * Waterline as a fraction up the water panel, and the rod's resting angle.
     *
     * <p>These are tightly coupled and were both wrong at first: with the surface at 86% the boat
     * and angler sat above the header and the rod ran off the top of the screen entirely. The
     * surface has to leave a full angler height plus the rod's vertical reach below the header,
     * and at the 9:16 design ratio that is the binding constraint on the whole scene.
     */
    private static final float SURFACE_FRACTION = 0.70f;
    private static final float ROD_REST_ANGLE = 22f;
    private static final float ROD_BEND_RANGE = 74f;
    private static final Color ROD_COLOR = new Color(0.36f, 0.26f, 0.18f, 1f);

    private float surfaceY() {
        float top = contentTop();
        float bottom = waterBottom();
        return bottom + (top - bottom) * SURFACE_FRACTION;
    }

    private void drawFightVisual() {
        FishingSession s = controller.session();
        float surface = surfaceY();
        float bottom = waterBottom();

        float pull = s != null && s.getPhase() == SessionPhase.FIGHT ? controller.lastPull() : 0f;
        boolean straining = pull > 0.45f;

        // --- Boat and angler -------------------------------------------------------------
        float boatW = 300f, boatH = 100f;
        float boatX = Theme.WORLD_WIDTH * ANGLER_X - boatW * 0.5f;
        float boatY = surface - boatH * 0.52f;
        game.batch.setColor(Color.WHITE);
        game.batch.draw(art.atlas.get("boat"), boatX, boatY, boatW, boatH);

        String pose = straining ? "angler_pull" : "angler_idle";
        float anglerW = 134f, anglerH = 176f;
        float anglerX = Theme.WORLD_WIDTH * ANGLER_X - anglerW * 0.5f;
        float anglerY = boatY + boatH * 0.34f;
        game.batch.draw(art.atlas.get(pose), anglerX, anglerY, anglerW, anglerH);

        float[] grip = art.atlas.anchor(pose + "_grip");
        // Anchors are in image space, where y runs downwards; the world has y up.
        float gripX = anglerX + grip[0] * anglerW;
        float gripY = anglerY + (1f - grip[1]) * anglerH;

        // --- Rod -------------------------------------------------------------------------
        float[] tip = drawRod(gripX, gripY, s == null ? 0f : shownTension);

        if (s == null) {
            ui.textCentered(art.font, "Đang chuẩn bị...", Theme.WORLD_WIDTH / 2f,
                    bottom + 220f, Theme.TEXT_DIM);
            return;
        }

        if (s.getPhase() == SessionPhase.SEARCHING) {
            float bobY = surface + (float) Math.sin(time * 3.1f) * 8f;
            drawLine(tip[0], tip[1], tip[0] + 26f, bobY, 3f, Theme.TEXT_DIM);
            game.batch.setColor(Theme.GOLD);
            game.batch.draw(art.circle, tip[0] + 26f - 17f, bobY - 17f, 34f, 34f);
            game.batch.setColor(Color.WHITE);
            ui.textCentered(art.fontSmall, "Đang chờ cá cắn câu...", Theme.WORLD_WIDTH / 2f,
                    surface - 90f, Theme.TEXT_DIM);
            return;
        }

        if (s.getPhase() == SessionPhase.BITE) {
            float pulse = 0.5f + 0.5f * (float) Math.sin(time * 24f);
            float bx = tip[0] + 26f;
            drawLine(tip[0], tip[1], bx, surface, 3f, Theme.DANGER);
            game.batch.setColor(Theme.DANGER.r, Theme.DANGER.g, Theme.DANGER.b,
                    0.4f + pulse * 0.6f);
            float size = 130f + pulse * 46f;
            game.batch.draw(art.ripple, bx - size / 2f, surface - size / 2f, size, size);
            game.batch.setColor(Color.WHITE);
            ui.textCentered(art.fontLarge, "CẮN CÂU!", Theme.WORLD_WIDTH / 2f,
                    surface - 110f, Theme.DANGER);
            return;
        }

        FishState fish = s.getFish();
        if (fish == null) return;

        // --- Fish ------------------------------------------------------------------------
        // Distance drives how far right and how deep the fish sits, so spool risk is legible
        // from the picture alone.
        float d = shownDistance;
        float fishX = Theme.WORLD_WIDTH * (0.44f + 0.38f * d);
        float swimTop = surface - 100f;
        float swimBottom = bottom + 100f;
        float fishY = swimTop - d * (swimTop - swimBottom);
        fishY += (float) Math.sin(time * (1.8f + fish.phase.driveMultiplier)) * 16f;

        Color tensionColor = Theme.tensionColor(shownTension, s.getSafeTensionRatio());
        drawLine(tip[0], tip[1], fishX + 40f, fishY, 4f, tensionColor);

        float scale = 0.80f + Math.min(1.5f, fish.weight / 55f);
        float fw = 210f * scale;
        float fh = fw * 0.5f;
        Color tint = Theme.rarityColor(fish.rarity);
        // A tired fish dims; a raging one flashes. Both read before the gauges do.
        float energy = 0.62f + 0.38f * fish.staminaRatio();
        float flash = fish.phase == com.vancan.autofishing.sim.FishPhase.RAGE
                ? 0.35f + 0.35f * (float) Math.sin(time * 18f) : 0f;
        game.batch.setColor(
                Math.min(1f, tint.r * energy + flash),
                Math.min(1f, tint.g * energy),
                Math.min(1f, tint.b * energy), 1f);
        game.batch.draw(art.fish(fish.species.archetype), fishX - fw / 2f, fishY - fh / 2f, fw, fh);
        game.batch.setColor(Color.WHITE);

        ui.textCentered(art.fontSmall, fish.species.name + "  ·  " + Ui.weight(fish.weight),
                fishX, fishY - fh / 2f - 12f, tint);
        ui.textCentered(art.fontSmall, fish.phase.displayName, fishX, fishY + fh / 2f + 46f,
                fish.phase.isOpening() ? Theme.GOOD : Theme.WARN);
    }

    /**
     * Draws the rod as a curve bending under load and returns its tip.
     *
     * <p>The bend is the clearest feedback the game has: it tracks tension directly, so a player
     * can read how close the line is to snapping without looking away from the water. Sampled as
     * a quadratic curve of tapering segments rather than baked as a sprite, because a baked rod
     * cannot bend.
     */
    private float[] drawRod(float gripX, float gripY, float bend) {
        // Unloaded the rod points up and forward; under load the tip is dragged down and out.
        float rad = (float) Math.toRadians(ROD_REST_ANGLE - ROD_BEND_RANGE * Ui.clamp01(bend));
        float tipX = gripX + ROD_LENGTH * (float) Math.cos(rad);
        float tipY = gripY + ROD_LENGTH * (float) Math.sin(rad);

        // Control point on the unbent line, which makes the curve bow rather than kink.
        float straight = (float) Math.toRadians(ROD_REST_ANGLE);
        float ctrlX = gripX + ROD_LENGTH * 0.55f * (float) Math.cos(straight);
        float ctrlY = gripY + ROD_LENGTH * 0.55f * (float) Math.sin(straight);

        int segments = 16;
        float px = gripX, py = gripY;
        for (int i = 1; i <= segments; i++) {
            float t = i / (float) segments;
            float mt = 1f - t;
            float x = mt * mt * gripX + 2f * mt * t * ctrlX + t * t * tipX;
            float y = mt * mt * gripY + 2f * mt * t * ctrlY + t * t * tipY;
            drawLine(px, py, x, y, 11f * (1f - t * 0.78f), ROD_COLOR);
            px = x;
            py = y;
        }
        return new float[]{px, py};
    }
    /** Draws a straight line by stretching and rotating the pixel region. */
    private void drawLine(float x1, float y1, float x2, float y2, float thickness, Color color) {
        float dx = x2 - x1, dy = y2 - y1;
        float length = (float) Math.sqrt(dx * dx + dy * dy);
        float angle = (float) Math.toDegrees(Math.atan2(dy, dx));
        game.batch.setColor(color);
        game.batch.draw(art.pixel, x1, y1 - thickness / 2f, 0f, thickness / 2f,
                length, thickness, 1f, 1f, angle);
        game.batch.setColor(Color.WHITE);
    }

    /**
     * Label above, bar below. Returns the bar's bottom edge to continue laying out from.
     *
     * <p>libGDX draws text downwards from the given y - that y is the top of the line, not the
     * baseline - so the bar has to clear a full line height. Assuming a baseline here is what put
     * every label on top of its own bar.
     */
    private float gaugeRow(String label, String value, Color valueColor,
                           float ratio, Color fill, float x, float y, float w, float barHeight) {
        ui.text(art.fontSmall, label, x, y, Theme.TEXT_DIM);
        if (value != null && !value.isEmpty()) {
            ui.textRight(art.fontSmall, value, x + w, y, valueColor);
        }
        float barY = y - art.fontSmall.getLineHeight() - 10f - barHeight;
        ui.bar(x, barY, w, barHeight, ratio, fill, Theme.PANEL_LIGHT);
        return barY;
    }

    private void drawGauges() {
        FishingSession s = controller.session();
        float x = Theme.PAD;
        float w = Theme.WORLD_WIDTH - Theme.PAD * 2f;
        float y = GAUGE_TOP;

        float safeRatio = s == null ? 0.7f : s.getSafeTensionRatio();
        Color tensionColor = Theme.tensionColor(shownTension, safeRatio);

        float barY = gaugeRow("Lực căng dây", Ui.percent(shownTension), tensionColor,
                shownTension, tensionColor, x, y, w, 36f);
        // The threshold marker is what makes the gauge actionable rather than decorative.
        ui.barMarker(x, barY, w, 36f, safeRatio, Theme.TEXT);
        y = barY - 34f;

        float integrity = s == null ? 1f : s.getLineIntegrity();
        Color integrityColor = integrity < 0.3f ? Theme.DANGER : Theme.GOOD;
        y = gaugeRow("Độ bền dây", Ui.percent(integrity),
                integrity < 0.3f ? Theme.DANGER : Theme.TEXT,
                integrity, integrityColor, x, y, w, 22f) - 34f;

        // Fish condition on the left, distance on the right.
        FishState fish = s == null ? null : s.getFish();
        float half = (w - 24f) / 2f;
        float rx = x + half + 24f;

        float leftBar = gaugeRow("Thể lực / sinh lực cá", "", Theme.TEXT,
                fish == null ? 0f : fish.staminaRatio(), Theme.WARN, x, y, half, 22f);
        // HP rides directly under stamina rather than taking a labelled row of its own: it is the
        // secondary win condition, and a fourth label row collided with the skill bar.
        ui.bar(x, leftBar - 16f, half, 12f, fish == null ? 0f : fish.hpRatio(),
                Theme.DANGER, Theme.PANEL_LIGHT);

        gaugeRow("Khoảng cách", s == null ? "—" : Math.round(s.getDistance()) + "m",
                shownDistance > 0.8f ? Theme.DANGER : Theme.TEXT,
                // Inverted: a full bar means the fish is close, which is the good direction.
                1f - shownDistance, Theme.ACCENT, rx, y, half, 22f);
        ui.text(art.fontSmall, s == null ? "" : "Thời gian " + Ui.duration(s.getElapsed()),
                rx, leftBar - 4f, Theme.TEXT_DIM);
        ui.textRight(art.fontSmall, fish == null ? "" : fish.phase.displayName,
                rx + half, leftBar - 4f, Theme.WARN);
    }

    private void drawControls() {
        float x = Theme.PAD;
        float w = Theme.WORLD_WIDTH - Theme.PAD * 2f;

        drawSkillBar(SKILL_Y);

        boolean auto = game.player.autoEnabled;
        float toggleW = w * 0.44f;
        if (ui.button(x, STRATEGY_ROW_Y, toggleW, ROW_H,
                auto ? "Auto: BẬT" : "Auto: TẮT", true, auto)) {
            controller.toggleAuto();
        }
        if (ui.button(x + toggleW + 20f, STRATEGY_ROW_Y, w - toggleW - 20f, ROW_H,
                game.player.autoStrategy.displayName, true, showStrategyPicker)) {
            showStrategyPicker = !showStrategyPicker;
        }

        if (showStrategyPicker) {
            drawStrategyPicker();
            return;
        }

        if (auto) {
            ui.textCentered(art.fontSmall, game.player.autoStrategy.summary,
                    Theme.WORLD_WIDTH / 2f, STRATEGY_ROW_Y - 18f, Theme.TEXT_DIM);
            if (controller.isIdle()
                    && ui.button(x, ACTION_ROW_Y, w, ROW_H, "Thả câu ngay")) {
                controller.cast();
            }
        } else if (controller.isIdle()) {
            ui.textCentered(art.fontSmall, "Thủ công: kéo thanh để điều khiển lực",
                    Theme.WORLD_WIDTH / 2f, STRATEGY_ROW_Y - 18f, Theme.TEXT_DIM);
            if (ui.button(x, ACTION_ROW_Y, w, ROW_H, "Thả câu")) controller.cast();
        } else {
            ui.text(art.fontSmall, "Kéo lên để tăng lực  ·  " + Ui.percent(controller.manualPull),
                    x, ACTION_ROW_Y + ROW_H + 26f, Theme.TEXT_DIM);
            controller.manualPull = ui.verticalSlider(x, ACTION_ROW_Y, w, ROW_H,
                    controller.manualPull, Theme.BUTTON_ACTIVE);
            ui.textCentered(art.font, "GHÌ CẦN", Theme.WORLD_WIDTH / 2f,
                    ACTION_ROW_Y + ROW_H / 2f + 10f, Theme.TEXT);
        }
    }

    private void drawStrategyPicker() {
        AutoStrategy[] all = AutoStrategy.values();
        float h = 92f;
        float panelH = all.length * (h + 10f) + 30f;
        float x = Theme.PAD;
        float w = Theme.WORLD_WIDTH - Theme.PAD * 2f;
        float y = contentBottom() + 320f;

        // Scrim: the picker sits over the live gauges, and without dimming them the panel reads
        // as a floating slab rather than a modal choice.
        ui.rect(0, contentBottom(), Theme.WORLD_WIDTH, contentHeight(), Color.BLACK, 0.55f);
        ui.panel(x, y, w, panelH);
        for (int i = 0; i < all.length; i++) {
            AutoStrategy s = all[i];
            float by = y + panelH - 20f - (i + 1) * (h + 10f) + 10f;
            boolean active = game.player.autoStrategy == s;
            if (ui.button(x + 14f, by, w - 28f, h, "", true, active)) {
                controller.setStrategy(s);
                showStrategyPicker = false;
            }
            ui.text(art.font, s.displayName, x + 34f, by + h - 24f,
                    active ? Theme.ACCENT : Theme.TEXT);
            // A filter strategy in a zone that has nothing matching it aborts every single fish
            // and looks identical to a broken game. Say so before the player picks it.
            boolean futile = rejectsWholeSpot(s);
            ui.text(art.fontSmall,
                    futile ? "⚠ Ngư trường này không có cá phù hợp" : s.description,
                    x + 34f, by + 30f, futile ? Theme.WARN : Theme.TEXT_DIM);
        }
    }

    /** True when the strategy's target filter rejects every species in the current spot. */
    private boolean rejectsWholeSpot(AutoStrategy strategy) {
        if (strategy.minRarity == null && strategy.minWeight <= 0f) return false;
        if (controller.spot() == null) return false;
        for (EncounterTable.Entry e : controller.spot()
                .buildTable(game.content.species).getEntries()) {
            boolean rarityOk = strategy.minRarity == null
                    || e.species.rarity.ordinal() >= strategy.minRarity.ordinal();
            boolean weightOk = strategy.minWeight <= 0f
                    || e.species.maxWeight >= strategy.minWeight;
            if (rarityOk && weightOk) return false;
        }
        return true;
    }

    private void drawSkillBar(float y) {
        SkillRuntime[] skills = controller.session() == null
                ? null : controller.session().getSkills();
        if (skills == null || skills.length == 0) return;

        // Skill names are two Vietnamese words; a square icon button clipped every one of them,
        // so the row spans the full width and each button gets real label space.
        float gap = 16f;
        float margin = Theme.PAD;
        float available = Theme.WORLD_WIDTH - margin * 2f - gap * (skills.length - 1);
        float bw = available / skills.length;
        float bh = Theme.TOUCH_MIN;

        for (int i = 0; i < skills.length; i++) {
            SkillRuntime s = skills[i];
            float bx = margin + i * (bw + gap);
            boolean ready = s.isReady();

            ui.rect(bx, y, bw, bh, ready ? Theme.BUTTON : Theme.BUTTON_DISABLED);
            if (s.isActive()) {
                ui.rect(bx, y, bw, bh, Theme.ACCENT, 0.45f);
            } else if (!ready) {
                // Cooldown drains from the top so the remaining fill reads as time left.
                ui.rect(bx, y, bw, bh * s.cooldownRatio(), Theme.PANEL, 0.85f);
            }
            ui.border(bx, y, bw, bh, 2f, ready ? Theme.ACCENT : Theme.BORDER);
            ui.textCentered(art.fontSmall, s.def.name, bx + bw / 2f, y + bh / 2f + 10f,
                    ready ? Theme.TEXT : Theme.TEXT_DIM);

            if (ready && ui.invisibleButton(bx, y, bw, bh)) {
                controller.requestSkill(i);
            }
        }
    }

    private void drawResultBanner() {
        FishRecord c = controller.lastCatch();
        float h = 150f;
        float y = waterBottom() + 20f;
        float x = Theme.PAD;
        float w = Theme.WORLD_WIDTH - Theme.PAD * 2f;

        ui.rect(x, y, w, h, Theme.PANEL);
        ui.border(x, y, w, h, 3f, c != null ? Theme.GOOD : Theme.DANGER);

        if (c != null) {
            String title = c.personalBest ? "KỶ LỤC MỚI!" : "Bắt được!";
            ui.text(art.font, title, x + 24f, y + h - 34f,
                    c.personalBest ? Theme.GOLD : Theme.GOOD);
            ui.text(art.fontSmall, controller.lastFishName() + "  ·  "
                            + Ui.weight(controller.lastFishWeight())
                            + "  ·  " + c.rarity.displayName,
                    x + 24f, y + h - 82f, Theme.rarityColor(c.rarity));
            ui.textRight(art.font, "+" + Ui.number(c.goldValue) + " ⧫", x + w - 24f,
                    y + h - 34f, Theme.GOLD);
            ui.textRight(art.fontSmall, "+" + c.xpValue + " KN", x + w - 24f,
                    y + h - 82f, Theme.ACCENT);
            if (controller.levelsGainedOnLastCatch() > 0) {
                ui.text(art.fontSmall, "Lên cấp! Bạn có điểm tiềm năng mới.",
                        x + 24f, y + 26f, Theme.GOLD);
            }
        } else {
            ui.text(art.font, controller.lastFailure().displayName, x + 24f, y + h - 34f,
                    Theme.DANGER);
            String name = controller.lastFishName();
            ui.text(art.fontSmall,
                    name == null ? "Thử lại với mồi tốt hơn."
                            : "Mất " + name + " (" + Ui.weight(controller.lastFishWeight()) + ")",
                    x + 24f, y + h - 82f, Theme.TEXT_DIM);
        }
    }

    private void drawOfflineReport() {
        OfflineSettlement.Result r = offlineReport;
        float w = Theme.WORLD_WIDTH - 120f;
        float h = 700f;
        float x = 60f;
        float y = (worldHeight() - h) / 2f;

        ui.rect(0, 0, Theme.WORLD_WIDTH, worldHeight(), Color.BLACK, 0.72f);
        ui.panel(x, y, w, h);

        // fontLarge overflowed the panel at this string length; the heading uses the body face.
        ui.textCentered(art.font, "Câu tự động khi vắng mặt",
                Theme.WORLD_WIDTH / 2f, y + h - 60f, Theme.ACCENT);
        ui.textCentered(art.fontSmall,
                Ui.trim1(r.hoursCredited) + " giờ được tính"
                        + (r.capped ? "  (đã chạm giới hạn)" : ""),
                Theme.WORLD_WIDTH / 2f, y + h - 130f,
                r.capped ? Theme.WARN : Theme.TEXT_DIM);

        float rowY = y + h - 210f;
        row("Số lần thả câu", Ui.number(r.casts), x + 40f, rowY, w - 80f, Theme.TEXT);
        row("Cá bắt được", Ui.number(r.catches), x + 40f, rowY - 70f, w - 80f, Theme.TEXT);
        row("Vàng", "+" + Ui.number(r.gold), x + 40f, rowY - 140f, w - 80f, Theme.GOLD);
        row("Kinh nghiệm", "+" + Ui.number(r.xp), x + 40f, rowY - 210f, w - 80f, Theme.ACCENT);

        if (r.capped) {
            ui.textCentered(art.fontSmall, "Nâng cấp trợ thủ để tăng giới hạn ngoại tuyến.",
                    Theme.WORLD_WIDTH / 2f, y + 150f, Theme.TEXT_DIM);
        }
        if (ui.button(x + 40f, y + 40f, w - 80f, Theme.TOUCH_MIN, "Nhận")) {
            offlineReport = null;
        }
    }

    private void row(String label, String value, float x, float y, float w, Color valueColor) {
        ui.text(art.font, label, x, y, Theme.TEXT_DIM);
        ui.textRight(art.font, value, x + w, y, valueColor);
    }
}
