package com.vancan.autofishing;

import com.vancan.autofishing.content.ContentLoader;
import com.vancan.autofishing.content.GameContent;
import com.vancan.autofishing.content.TextSource;

import java.io.File;
import java.io.IOException;
import java.nio.charset.StandardCharsets;
import java.nio.file.Files;

/** Loads the real shipped content tables from disk, once, for every test that needs them. */
public final class TestContent {

    private static GameContent cached;

    private TestContent() {
    }

    public static synchronized GameContent get() {
        if (cached == null) {
            cached = new ContentLoader().load(diskSource());
        }
        return cached;
    }

    public static TextSource diskSource() {
        final File root = dataRoot();
        return new TextSource() {
            @Override
            public String read(String path) {
                try {
                    return new String(Files.readAllBytes(new File(root, path).toPath()),
                            StandardCharsets.UTF_8);
                } catch (IOException e) {
                    throw new IllegalStateException("Cannot read content file: " + path, e);
                }
            }
        };
    }

    /** Gradle passes the root explicitly; the walk-up keeps this working from an IDE. */
    private static File dataRoot() {
        String configured = System.getProperty("vancan.dataRoot");
        if (configured != null && new File(configured).isDirectory()) {
            return new File(configured);
        }
        File dir = new File("").getAbsoluteFile();
        for (int i = 0; i < 5 && dir != null; i++) {
            File candidate = new File(dir, "assets/data");
            if (candidate.isDirectory()) return candidate;
            dir = dir.getParentFile();
        }
        throw new IllegalStateException("Could not locate assets/data from " + new File("").getAbsolutePath());
    }
}
