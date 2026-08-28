package com.vancan.autofishing.platform;

import com.badlogic.gdx.Gdx;
import com.badlogic.gdx.Preferences;
import com.vancan.autofishing.meta.PlayerState;
import com.vancan.autofishing.meta.SaveGame;

/**
 * Persistence.
 *
 * <p>Uses {@code Preferences} rather than a file because it is the one storage API with a working
 * implementation on all four targets - including HTML5, where it maps onto local storage and a
 * plain file write is not available. The payload is the JSON blob from {@link SaveGame}, so the
 * format is identical everywhere and can be lifted to a server save later without changing
 * anything above this class.
 */
public final class SaveStore {

    private static final String PREFS = "van-can-auto-fishing";
    private static final String KEY = "player";

    public PlayerState load() {
        try {
            Preferences prefs = Gdx.app.getPreferences(PREFS);
            String text = prefs.getString(KEY, null);
            if (text == null || text.isEmpty()) return null;
            return SaveGame.load(text);
        } catch (Exception e) {
            // A corrupt or future-version save must not brick the app: log it and start fresh
            // rather than crashing on every launch with no way back.
            Gdx.app.error("SaveStore", "Could not load save, starting a new game", e);
            return null;
        }
    }

    public void save(PlayerState player) {
        try {
            Preferences prefs = Gdx.app.getPreferences(PREFS);
            prefs.putString(KEY, SaveGame.save(player));
            prefs.flush();
        } catch (Exception e) {
            Gdx.app.error("SaveStore", "Could not write save", e);
        }
    }

    public void clear() {
        Preferences prefs = Gdx.app.getPreferences(PREFS);
        prefs.remove(KEY);
        prefs.flush();
    }
}
