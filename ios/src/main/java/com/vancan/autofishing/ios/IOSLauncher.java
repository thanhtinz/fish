package com.vancan.autofishing.ios;

import com.badlogic.gdx.backends.iosrobovm.IOSApplication;
import com.badlogic.gdx.backends.iosrobovm.IOSApplicationConfiguration;
import com.vancan.autofishing.VanCanGame;
import org.robovm.apple.foundation.NSAutoreleasePool;
import org.robovm.apple.uikit.UIApplication;

/** iOS launcher. Orientation is locked to portrait in Info.plist.xml (GDD 16). */
public class IOSLauncher extends IOSApplication.Delegate {

    @Override
    protected IOSApplication createApplication() {
        IOSApplicationConfiguration config = new IOSApplicationConfiguration();
        // Render at the device's native resolution; the ExtendViewport handles the aspect ratio.
        config.useAccelerometer = false;
        config.useCompass = false;
        config.hideHomeIndicator = true;
        return new IOSApplication(new VanCanGame(), config);
    }

    public static void main(String[] argv) {
        NSAutoreleasePool pool = new NSAutoreleasePool();
        UIApplication.main(argv, null, IOSLauncher.class);
        pool.close();
    }
}
