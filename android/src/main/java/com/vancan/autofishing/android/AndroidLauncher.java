package com.vancan.autofishing.android;

import android.os.Bundle;

import com.badlogic.gdx.backends.android.AndroidApplication;
import com.badlogic.gdx.backends.android.AndroidApplicationConfiguration;
import com.vancan.autofishing.VanCanGame;

/** Android launcher. Orientation is locked to portrait in the manifest (GDD 16). */
public class AndroidLauncher extends AndroidApplication {

    @Override
    protected void onCreate(Bundle savedInstanceState) {
        super.onCreate(savedInstanceState);
        AndroidApplicationConfiguration config = new AndroidApplicationConfiguration();
        config.useImmersiveMode = true;
        config.useAccelerometer = false;
        config.useCompass = false;
        // The game has no audio yet; leaving the audio subsystem off avoids requesting the
        // permissions and buffers that come with it.
        config.disableAudio = true;
        initialize(new VanCanGame(), config);
    }
}
