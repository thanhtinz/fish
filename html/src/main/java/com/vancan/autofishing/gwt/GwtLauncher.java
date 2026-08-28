package com.vancan.autofishing.gwt;

import com.badlogic.gdx.ApplicationListener;
import com.badlogic.gdx.backends.gwt.GwtApplication;
import com.badlogic.gdx.backends.gwt.GwtApplicationConfiguration;
import com.vancan.autofishing.VanCanGame;

/** HTML5 launcher. */
public class GwtLauncher extends GwtApplication {

    @Override
    public GwtApplicationConfiguration getConfig() {
        // Fill the browser window and let the ExtendViewport handle the aspect ratio, so the game
        // works both in a phone browser and in a desktop tab without letterboxing.
        GwtApplicationConfiguration config = new GwtApplicationConfiguration(true);
        config.padVertical = 0;
        config.padHorizontal = 0;
        return config;
    }

    @Override
    public ApplicationListener createApplicationListener() {
        return new VanCanGame();
    }
}
