package com.vancan.autofishing.lwjgl3;

import com.badlogic.gdx.backends.lwjgl3.Lwjgl3Application;
import com.badlogic.gdx.backends.lwjgl3.Lwjgl3ApplicationConfiguration;
import com.vancan.autofishing.VanCanGame;

/**
 * Desktop launcher. This is the development harness, not a shipping target: the game is designed
 * for portrait phones, so the window is opened at a phone aspect ratio rather than maximised.
 */
public final class DesktopLauncher {

    private DesktopLauncher() {
    }

    public static void main(String[] args) {
        Lwjgl3ApplicationConfiguration config = new Lwjgl3ApplicationConfiguration();
        config.setTitle("Vạn Cân: Auto Fishing");
        // 9:16 at a size that fits a laptop screen; matches the 1080x1920 design resolution.
        config.setWindowedMode(495, 880);
        config.setForegroundFPS(60);
        config.useVsync(true);
        config.setWindowSizeLimits(320, 480, -1, -1);

        ScreenshotHarness harness = ScreenshotHarness.fromArgs(args);
        new Lwjgl3Application(harness == null ? new VanCanGame() : harness, config);
    }
}
