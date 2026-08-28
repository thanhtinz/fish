package com.vancan.autofishing.ui;

import com.badlogic.gdx.graphics.Color;
import com.badlogic.gdx.graphics.Pixmap;
import com.badlogic.gdx.graphics.Texture;
import com.badlogic.gdx.graphics.g2d.BitmapFont;
import com.badlogic.gdx.graphics.g2d.TextureRegion;
import com.badlogic.gdx.utils.Disposable;

/**
 * All game art.
 *
 * <p>Two layers. Character, fish and portrait sprites come from an atlas baked offline by
 * {@code tools/SpriteGen.java}; the primitives below (discs, ripples, gradients) are still
 * generated at start-up because they are stretched and tinted per frame and would gain nothing
 * from being baked.
 *
 * <p>Everything is drawn for this project. The reference title's assets are copyrighted, and the
 * brief (GDD 2.2) forbids reusing another game's artwork - so no sprite here derives from one.
 */
public final class Art implements Disposable {

    /** Baked sprites: fish, the angler, the boat, team portraits. */
    public Atlas atlas;

    /** 1x1 white pixel; every solid rectangle and bar is drawn by stretching this. */
    public TextureRegion pixel;
    public TextureRegion circle;
    public TextureRegion softCircle;
    public TextureRegion ripple;
    public TextureRegion bubble;
    public TextureRegion verticalFade;

    public BitmapFont font;
    public BitmapFont fontSmall;
    public BitmapFont fontLarge;

    private final com.badlogic.gdx.utils.Array<Texture> textures =
            new com.badlogic.gdx.utils.Array<Texture>();

    public void load() {
        atlas = new Atlas();
        atlas.load("sprites/atlas.png", "sprites/atlas.json");

        pixel = region(solid(4, 4, Color.WHITE));
        circle = region(circle(128, false));
        softCircle = region(circle(128, true));
        ripple = region(ring(128, 0.72f));
        bubble = region(ring(64, 0.55f));
        verticalFade = region(verticalFade(4, 256));

        font = loadFont(1f);
        fontSmall = loadFont(0.72f);
        fontLarge = loadFont(1.45f);
    }

    private BitmapFont loadFont(float scale) {
        BitmapFont f = new BitmapFont(
                com.badlogic.gdx.Gdx.files.internal("fonts/game.fnt"),
                com.badlogic.gdx.Gdx.files.internal("fonts/game.png"), false);
        // The atlas is baked at 32px and the world is 1080 wide, so everything is scaled up.
        // Linear filtering keeps that readable instead of blocky.
        f.getRegion().getTexture().setFilter(Texture.TextureFilter.Linear,
                Texture.TextureFilter.Linear);
        f.getData().setScale(scale * 1.6f);
        f.setUseIntegerPositions(false);
        return f;
    }

    private TextureRegion region(Pixmap pixmap) {
        Texture texture = new Texture(pixmap);
        texture.setFilter(Texture.TextureFilter.Linear, Texture.TextureFilter.Linear);
        pixmap.dispose();
        textures.add(texture);
        return new TextureRegion(texture);
    }

    private Pixmap solid(int w, int h, Color color) {
        Pixmap p = new Pixmap(w, h, Pixmap.Format.RGBA8888);
        p.setColor(color);
        p.fill();
        return p;
    }

    private Pixmap circle(int size, boolean soft) {
        Pixmap p = new Pixmap(size, size, Pixmap.Format.RGBA8888);
        p.setBlending(Pixmap.Blending.None);
        float r = size / 2f;
        for (int y = 0; y < size; y++) {
            for (int x = 0; x < size; x++) {
                float dx = x - r + 0.5f, dy = y - r + 0.5f;
                float d = (float) Math.sqrt(dx * dx + dy * dy) / r;
                float a = soft ? Math.max(0f, 1f - d * d) : (d <= 1f ? smoothEdge(d, r) : 0f);
                p.setColor(1f, 1f, 1f, a);
                p.drawPixel(x, y);
            }
        }
        return p;
    }

    private Pixmap ring(int size, float inner) {
        Pixmap p = new Pixmap(size, size, Pixmap.Format.RGBA8888);
        p.setBlending(Pixmap.Blending.None);
        float r = size / 2f;
        for (int y = 0; y < size; y++) {
            for (int x = 0; x < size; x++) {
                float dx = x - r + 0.5f, dy = y - r + 0.5f;
                float d = (float) Math.sqrt(dx * dx + dy * dy) / r;
                float a = 0f;
                if (d <= 1f && d >= inner) {
                    float t = (d - inner) / (1f - inner);
                    a = (float) Math.sin(t * Math.PI);
                }
                p.setColor(1f, 1f, 1f, a);
                p.drawPixel(x, y);
            }
        }
        return p;
    }

    private Pixmap verticalFade(int w, int h) {
        Pixmap p = new Pixmap(w, h, Pixmap.Format.RGBA8888);
        p.setBlending(Pixmap.Blending.None);
        for (int y = 0; y < h; y++) {
            float a = 1f - (y / (float) (h - 1));
            p.setColor(1f, 1f, 1f, a * a);
            p.drawLine(0, y, w, y);
        }
        return p;
    }

    /** Anti-aliases the last pixel of a circle edge so shapes do not look jagged when scaled. */
    private float smoothEdge(float d, float radius) {
        float edge = 1f / radius;
        return d > 1f - edge ? (1f - d) / edge : 1f;
    }

    /**
     * Sprite for a fish.
     *
     * <p>Keyed on behaviour archetype, because the silhouette is what tells a player how the
     * fight will go - except at the top rarity tiers, which get their own sprite so a legendary
     * catch is visibly not the same creature as the common fish sharing its archetype.
     */
    public TextureRegion fish(com.vancan.autofishing.sim.Archetype archetype,
                              com.vancan.autofishing.sim.Rarity rarity) {
        if (rarity != null
                && rarity.ordinal() >= com.vancan.autofishing.sim.Rarity.LEGENDARY.ordinal()) {
            return atlas.get("fish_legendary");
        }
        return atlas.get("fish_" + archetype.name().toLowerCase());
    }

    /** Portrait for a recruitable angler, falling back to the first if content adds a new one. */
    public TextureRegion portrait(String anglerId) {
        String key = "portrait_" + anglerId;
        return atlas.has(key) ? atlas.get(key) : atlas.get("portrait_ag_lam");
    }

    @Override
    public void dispose() {
        if (atlas != null) atlas.dispose();
        for (Texture t : textures) t.dispose();
        textures.clear();
        if (font != null) font.dispose();
        if (fontSmall != null) fontSmall.dispose();
        if (fontLarge != null) fontLarge.dispose();
    }
}
