package com.vancan.autofishing.screen;

import com.badlogic.gdx.Gdx;
import com.badlogic.gdx.ScreenAdapter;
import com.badlogic.gdx.graphics.GL20;
import com.vancan.autofishing.VanCanGame;
import com.vancan.autofishing.meta.Currency;
import com.vancan.autofishing.meta.PlayerState;
import com.vancan.autofishing.ui.Art;
import com.vancan.autofishing.ui.Theme;
import com.vancan.autofishing.ui.Ui;

/**
 * Shared portrait chrome: a status header at the top and a navigation bar at the bottom, with the
 * screen's own content in between.
 *
 * <p>The nav bar sits at the bottom on purpose - GDD 16 calls for one-handed play, and the bottom
 * third of a tall phone is the only region a thumb reaches comfortably.
 */
public abstract class BaseScreen extends ScreenAdapter {

    protected final VanCanGame game;
    protected final Ui ui;
    protected final Art art;

    public static final float HEADER_HEIGHT = 150f;
    public static final float NAV_HEIGHT = 160f;

    protected BaseScreen(VanCanGame game) {
        this.game = game;
        this.ui = game.ui;
        this.art = game.art;
    }

    /**
     * Actual world height, which is taller than the design height on screens narrower than 9:16.
     *
     * <p>ExtendViewport keeps the design width and grows the world vertically to fill the display.
     * Anchoring top-of-screen chrome to the fixed design height instead left a black band above
     * the header on a 414x896 phone - the extra world was simply never drawn into.
     */
    protected float worldHeight() {
        return game.viewport.getWorldHeight();
    }

    /** Content area top edge. */
    protected float contentTop() {
        return worldHeight() - HEADER_HEIGHT;
    }

    /** Content area bottom edge. */
    protected float contentBottom() {
        return NAV_HEIGHT;
    }

    protected float contentHeight() {
        return contentTop() - contentBottom();
    }

    @Override
    public final void render(float delta) {
        // Clamp the frame delta: a stall (a GC pause, a backgrounded tab resuming) would otherwise
        // hand the simulation a huge step and fast-forward a whole fight in one frame.
        float dt = Math.min(delta, 0.25f);
        update(dt);

        Gdx.gl.glClearColor(Theme.SKY_TOP.r, Theme.SKY_TOP.g, Theme.SKY_TOP.b, 1f);
        Gdx.gl.glClear(GL20.GL_COLOR_BUFFER_BIT);

        game.viewport.apply();
        game.batch.setProjectionMatrix(game.viewport.getCamera().combined);
        ui.begin(game.viewport);
        game.batch.begin();

        drawContent(dt);
        drawHeader();
        drawNav();

        game.batch.end();
    }

    protected void update(float delta) {
    }

    protected abstract void drawContent(float delta);

    /** Screen title shown in the header. */
    protected abstract String title();

    private void drawHeader() {
        float top = worldHeight();
        float y = top - HEADER_HEIGHT;
        ui.rect(0, y, Theme.WORLD_WIDTH, HEADER_HEIGHT, Theme.PANEL);
        ui.rect(0, y, Theme.WORLD_WIDTH, 3f, Theme.BORDER);

        PlayerState p = game.player;
        ui.text(art.font, title(), Theme.PAD, top - 40f, Theme.TEXT);
        ui.text(art.fontSmall, "Cấp " + p.level + "  ·  " + p.discoveredSpecies() + "/"
                        + game.content.species.size() + " loài",
                Theme.PAD, top - 100f, Theme.TEXT_DIM);

        String gold = Ui.number(p.get(Currency.GOLD)) + " ⧫";
        ui.textRight(art.font, gold, Theme.WORLD_WIDTH - Theme.PAD, top - 40f, Theme.GOLD);
        ui.textRight(art.fontSmall, Ui.number(p.get(Currency.GEMS)) + " ◆",
                Theme.WORLD_WIDTH - Theme.PAD, top - 100f, Theme.ACCENT);

        // XP bar hugging the bottom edge of the header.
        float need = PlayerState.xpForLevel(p.level);
        ui.bar(Theme.PAD, y + 14f, Theme.WORLD_WIDTH - Theme.PAD * 2f, 10f,
                need <= 0 ? 0 : p.xp / need, Theme.ACCENT, Theme.PANEL_LIGHT);
    }

    private static final String[] NAV_LABELS = {"Câu cá", "Bản đồ", "Ngư cụ", "Đội", "Đồ giám"};

    private void drawNav() {
        ui.rect(0, 0, Theme.WORLD_WIDTH, NAV_HEIGHT, Theme.PANEL);
        ui.rect(0, NAV_HEIGHT - 3f, Theme.WORLD_WIDTH, 3f, Theme.BORDER);

        float w = Theme.WORLD_WIDTH / NAV_LABELS.length;
        int current = navIndex();
        for (int i = 0; i < NAV_LABELS.length; i++) {
            float x = i * w;
            boolean active = i == current;
            if (active) {
                ui.rect(x + 8f, 12f, w - 16f, NAV_HEIGHT - 28f, Theme.BUTTON_ACTIVE, 0.35f);
                ui.rect(x + 8f, NAV_HEIGHT - 18f, w - 16f, 5f, Theme.ACCENT);
            }
            ui.textCentered(art.fontSmall, NAV_LABELS[i], x + w / 2f, NAV_HEIGHT / 2f + 10f,
                    active ? Theme.ACCENT : Theme.TEXT_DIM);

            if (!active && ui.invisibleButton(x, 0, w, NAV_HEIGHT)) {
                navigate(i);
            }
        }
    }

    protected abstract int navIndex();

    protected void navigate(int index) {
        switch (index) {
            case 0: game.setScreen(new FishingScreen(game)); break;
            case 1: game.setScreen(new MapScreen(game)); break;
            case 2: game.setScreen(new GearScreen(game)); break;
            case 3: game.setScreen(new TeamScreen(game)); break;
            case 4: game.setScreen(new CodexScreen(game)); break;
            default: break;
        }
    }
}
