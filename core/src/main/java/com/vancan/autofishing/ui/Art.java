package com.vancan.autofishing.ui;

import com.badlogic.gdx.graphics.Color;
import com.badlogic.gdx.graphics.Pixmap;
import com.badlogic.gdx.graphics.Texture;
import com.badlogic.gdx.graphics.g2d.BitmapFont;
import com.badlogic.gdx.graphics.g2d.TextureRegion;
import com.badlogic.gdx.utils.Disposable;

/**
 * All game art, generated procedurally at start-up.
 *
 * <p>The reference material supplied with this project was third-party artwork of unclear
 * provenance, and the design brief explicitly forbids reusing another game's assets. Generating
 * the art from code keeps the build free of licensing risk, keeps the download small, and means
 * every target - including the HTML5 build, where large binary assets hurt most - ships the same
 * visuals with no atlas pipeline. Replace this class with a real atlas when art direction lands.
 */
public final class Art implements Disposable {

    /** 1x1 white pixel; every solid rectangle and bar is drawn by stretching this. */
    public TextureRegion pixel;
    public TextureRegion circle;
    public TextureRegion softCircle;
    public TextureRegion ripple;
    public TextureRegion fishBody;
    public TextureRegion bubble;
    public TextureRegion verticalFade;

    public BitmapFont font;
    public BitmapFont fontSmall;
    public BitmapFont fontLarge;

    private final com.badlogic.gdx.utils.Array<Texture> textures =
            new com.badlogic.gdx.utils.Array<Texture>();

    public void load() {
        pixel = region(solid(4, 4, Color.WHITE));
        circle = region(circle(128, false));
        softCircle = region(circle(128, true));
        ripple = region(ring(128, 0.72f));
        fishBody = region(fish(256, 128));
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

    /**
     * A generic fish silhouette: an ellipse body with a swept tail. Species are differentiated by
     * tint and scale at draw time rather than by separate sprites.
     */
    private Pixmap fish(int w, int h) {
        Pixmap p = new Pixmap(w, h, Pixmap.Format.RGBA8888);
        p.setBlending(Pixmap.Blending.None);
        float bodyCx = w * 0.58f, bodyCy = h * 0.5f;
        float bodyRx = w * 0.36f, bodyRy = h * 0.30f;

        for (int y = 0; y < h; y++) {
            for (int x = 0; x < w; x++) {
                float a = 0f;

                float nx = (x - bodyCx) / bodyRx;
                float ny = (y - bodyCy) / bodyRy;
                float body = nx * nx + ny * ny;
                if (body <= 1f) {
                    a = Math.min(1f, (1f - body) * 6f);
                }

                // Tail: a triangle sweeping back from the body towards the left edge.
                float tailStart = w * 0.24f, tailEnd = bodyCx - bodyRx * 0.75f;
                if (x >= tailStart && x <= tailEnd) {
                    float t = (x - tailStart) / Math.max(1f, tailEnd - tailStart);
                    float halfHeight = h * (0.34f - 0.24f * t);
                    if (Math.abs(y - bodyCy) <= halfHeight) {
                        a = Math.max(a, 1f);
                    }
                }

                // Dorsal fin.
                float finCx = bodyCx - bodyRx * 0.1f;
                if (Math.abs(x - finCx) < bodyRx * 0.42f) {
                    float t = 1f - Math.abs(x - finCx) / (bodyRx * 0.42f);
                    float top = bodyCy - bodyRy - h * 0.14f * t;
                    if (y >= top && y <= bodyCy) a = Math.max(a, 1f);
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

    @Override
    public void dispose() {
        for (Texture t : textures) t.dispose();
        textures.clear();
        if (font != null) font.dispose();
        if (fontSmall != null) fontSmall.dispose();
        if (fontLarge != null) fontLarge.dispose();
    }
}
