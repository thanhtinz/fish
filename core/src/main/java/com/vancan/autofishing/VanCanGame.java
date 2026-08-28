package com.vancan.autofishing;

import com.badlogic.gdx.Game;
import com.badlogic.gdx.Gdx;
import com.badlogic.gdx.graphics.g2d.SpriteBatch;
import com.badlogic.gdx.utils.viewport.ExtendViewport;
import com.badlogic.gdx.utils.viewport.Viewport;
import com.vancan.autofishing.content.ContentLoader;
import com.vancan.autofishing.content.GameContent;
import com.vancan.autofishing.content.TextSource;
import com.vancan.autofishing.meta.OfflineSettlement;
import com.vancan.autofishing.meta.LoadoutResolver;
import com.vancan.autofishing.meta.PlayerFactory;
import com.vancan.autofishing.meta.PlayerState;
import com.vancan.autofishing.platform.SaveStore;
import com.vancan.autofishing.screen.FishingScreen;
import com.vancan.autofishing.ui.Art;
import com.vancan.autofishing.ui.Theme;
import com.vancan.autofishing.ui.Ui;

/** Application entry point shared by the desktop, HTML5, Android and iOS launchers. */
public class VanCanGame extends Game {

    public GameContent content;
    public PlayerState player;
    public SaveStore saveStore;

    public Art art;
    public SpriteBatch batch;
    public Ui ui;
    public Viewport viewport;

    /** Settlement produced at launch, shown once by the fishing screen then cleared. */
    public OfflineSettlement.Result pendingOfflineReport;

    private float autosaveTimer;
    private static final float AUTOSAVE_INTERVAL = 20f;

    @Override
    public void create() {
        batch = new SpriteBatch();
        art = new Art();
        art.load();
        ui = new Ui(art, batch);

        // Extend rather than fit: the design is authored for 1080x1920 but real phones run
        // anywhere from 18:9 to 21:9, and letterboxing a portrait game wastes the screen the
        // simulation is supposed to fill (GDD 16).
        viewport = new ExtendViewport(Theme.WORLD_WIDTH, Theme.WORLD_HEIGHT);

        content = new ContentLoader().load(new TextSource() {
            @Override
            public String read(String path) {
                return Gdx.files.internal("data/" + path).readString("UTF-8");
            }
        });

        saveStore = new SaveStore();
        long now = nowMillis();
        player = saveStore.load();
        if (player == null) {
            player = PlayerFactory.newPlayer(content, now);
        } else {
            settleOffline(now);
        }
        player.lastSeenAt = now;

        setScreen(new FishingScreen(this));
    }

    /** GDD 12: credit idle time on launch, capped, using the expected-value model. */
    private void settleOffline(long now) {
        long elapsed = now - player.lastSeenAt;
        if (elapsed <= 0) return;
        OfflineSettlement.Result result = OfflineSettlement.settle(
                player, content, LoadoutResolver.resolve(player, content), elapsed, now);
        if (result != null && OfflineSettlement.apply(player, result)) {
            pendingOfflineReport = result;
        }
    }

    /**
     * Wall clock in millis. On GWT {@code System.currentTimeMillis} is emulated but
     * {@code nanoTime} is not, so this is the one time source the whole game uses.
     */
    public static long nowMillis() {
        return System.currentTimeMillis();
    }

    @Override
    public void render() {
        super.render();

        autosaveTimer += Gdx.graphics.getDeltaTime();
        if (autosaveTimer >= AUTOSAVE_INTERVAL) {
            autosaveTimer = 0f;
            saveNow();
        }
    }

    public void saveNow() {
        player.lastSeenAt = nowMillis();
        saveStore.save(player);
    }

    @Override
    public void pause() {
        // Mobile can kill the process straight from the background, so this is the last reliable
        // chance to persist - autosave alone would lose up to a full interval of progress.
        saveNow();
        super.pause();
    }

    @Override
    public void resize(int width, int height) {
        viewport.update(width, height, true);
        super.resize(width, height);
    }

    @Override
    public void dispose() {
        saveNow();
        if (getScreen() != null) getScreen().dispose();
        art.dispose();
        batch.dispose();
    }
}
