package com.vancan.autofishing.lwjgl3;

import com.badlogic.gdx.Gdx;
import com.badlogic.gdx.graphics.Pixmap;
import com.badlogic.gdx.graphics.PixmapIO;
import com.vancan.autofishing.VanCanGame;
import com.vancan.autofishing.screen.CodexScreen;
import com.vancan.autofishing.screen.FishingScreen;
import com.vancan.autofishing.screen.GearScreen;
import com.vancan.autofishing.screen.MapScreen;
import com.vancan.autofishing.screen.TeamScreen;

/**
 * Runs the real game for a fixed number of frames, captures each screen and exits.
 *
 * <p>This exists so the UI can be verified automatically - under Xvfb in CI, or locally before a
 * release - instead of relying on someone remembering to look at every screen after a layout
 * change. Enabled with {@code --screenshot <output-dir>}; without it the launcher starts the game
 * normally.
 */
public final class ScreenshotHarness extends VanCanGame {

    private final String outputDir;
    private int frame;
    private int shotIndex;

    /**
     * Frames to run before each capture. The fishing shots wait much longer than the menu shots:
     * a bite averages 4.5 seconds, so a short warm-up would only ever photograph an empty pond
     * and the fight HUD - the part most worth reviewing - would never appear.
     */
    private static final int[] WARMUP_FRAMES = {420, 40, 40, 40, 40, 900};

    private ScreenshotHarness(String outputDir) {
        this.outputDir = outputDir;
    }

    static ScreenshotHarness fromArgs(String[] args) {
        for (int i = 0; i < args.length; i++) {
            if ("--screenshot".equals(args[i]) && i + 1 < args.length) {
                return new ScreenshotHarness(args[i + 1]);
            }
        }
        return null;
    }

    @Override
    public void render() {
        super.render();
        frame++;
        int required = WARMUP_FRAMES[Math.min(shotIndex, WARMUP_FRAMES.length - 1)];
        if (frame < required) return;
        frame = 0;

        capture(name(shotIndex));
        shotIndex++;

        switch (shotIndex) {
            case 1: setScreen(new MapScreen(this)); break;
            case 2: setScreen(new GearScreen(this)); break;
            case 3: setScreen(new TeamScreen(this)); break;
            case 4: setScreen(new CodexScreen(this)); break;
            case 5: setScreen(new FishingScreen(this)); break;
            default:
                Gdx.app.exit();
        }
    }

    private String name(int index) {
        switch (index) {
            case 0: return "01-fishing";
            case 1: return "02-map";
            case 2: return "03-gear";
            case 3: return "04-team";
            case 4: return "05-codex";
            default: return "06-fishing-later";
        }
    }

    private void capture(String name) {
        int w = Gdx.graphics.getBackBufferWidth();
        int h = Gdx.graphics.getBackBufferHeight();
        Pixmap raw = com.badlogic.gdx.utils.ScreenUtils.getFrameBufferPixmap(0, 0, w, h);
        // OpenGL reads the framebuffer bottom-up, so the raw pixmap is upside down.
        Pixmap flipped = new Pixmap(w, h, raw.getFormat());
        for (int y = 0; y < h; y++) {
            flipped.drawPixmap(raw, 0, y, 0, h - 1 - y, w, 1);
        }
        PixmapIO.writePNG(Gdx.files.absolute(outputDir + "/" + name + ".png"), flipped);
        raw.dispose();
        flipped.dispose();
        Gdx.app.log("Screenshot", "wrote " + name + ".png");
    }
}
