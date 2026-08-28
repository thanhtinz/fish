package com.vancan.autofishing.ui;

import com.badlogic.gdx.Gdx;
import com.badlogic.gdx.graphics.Texture;
import com.badlogic.gdx.graphics.g2d.TextureRegion;
import com.badlogic.gdx.utils.Disposable;
import com.badlogic.gdx.utils.JsonReader;
import com.badlogic.gdx.utils.JsonValue;

import java.util.HashMap;
import java.util.Map;

/**
 * The sprite sheet baked by {@code tools/SpriteGen.java}.
 *
 * <p>Deliberately not libGDX's {@code TextureAtlas}: that format is produced by the texture packer
 * and carries rotation, whitespace stripping and nine-patch data none of which this project has.
 * One PNG plus a small JSON of rectangles is the whole requirement, and it keeps the generator
 * readable as the single source of truth for what a sprite is.
 */
public final class Atlas implements Disposable {

    private Texture texture;
    private final Map<String, TextureRegion> regions = new HashMap<String, TextureRegion>();
    /** Named points in 0..1 sprite space, e.g. where the angler's hand grips the rod. */
    private final Map<String, float[]> anchors = new HashMap<String, float[]>();

    public void load(String pngPath, String jsonPath) {
        texture = new Texture(Gdx.files.internal(pngPath), true);
        // Mipmaps plus linear filtering: sprites are drawn well below their baked size on a phone,
        // and without mipmaps the fish edges crawl badly while they swim.
        texture.setFilter(Texture.TextureFilter.MipMapLinearLinear, Texture.TextureFilter.Linear);

        JsonValue root = new JsonReader().parse(Gdx.files.internal(jsonPath));

        JsonValue regionNode = root.get("regions");
        for (JsonValue r = regionNode.child; r != null; r = r.next) {
            int[] v = r.asIntArray();
            regions.put(r.name, new TextureRegion(texture, v[0], v[1], v[2], v[3]));
        }

        JsonValue anchorNode = root.get("anchors");
        if (anchorNode != null) {
            for (JsonValue a = anchorNode.child; a != null; a = a.next) {
                anchors.put(a.name, a.asFloatArray());
            }
        }
    }

    public TextureRegion get(String name) {
        TextureRegion region = regions.get(name);
        if (region == null) {
            throw new IllegalArgumentException("No sprite '" + name + "' in the atlas");
        }
        return region;
    }

    public boolean has(String name) {
        return regions.containsKey(name);
    }

    /** @return the anchor in 0..1 sprite space, or {@code null} */
    public float[] anchor(String name) {
        return anchors.get(name);
    }

    @Override
    public void dispose() {
        if (texture != null) texture.dispose();
    }
}
