package com.vancan.autofishing.ui;

import com.badlogic.gdx.Gdx;
import com.badlogic.gdx.graphics.Color;
import com.badlogic.gdx.graphics.g2d.BitmapFont;
import com.badlogic.gdx.graphics.g2d.GlyphLayout;
import com.badlogic.gdx.graphics.g2d.SpriteBatch;
import com.badlogic.gdx.math.Vector3;
import com.badlogic.gdx.utils.viewport.Viewport;

/**
 * A small immediate-mode widget layer.
 *
 * <p>Scene2D would mean a skin pipeline and a widget tree for what is, in this game, a handful of
 * rectangles that change every frame from simulation state. Immediate mode keeps the HUD code
 * beside the state it reads, which matters most on the fishing screen where the whole point is
 * that the display tracks the fight tick by tick.
 */
public final class Ui {

    private final Art art;
    private final SpriteBatch batch;
    private final GlyphLayout layout = new GlyphLayout();
    private final Vector3 touch = new Vector3();

    private Viewport viewport;
    private float pointerX, pointerY;
    private boolean pressed;
    private boolean justReleased;
    private float lastPointerY;
    private float dragDeltaY;
    private float dragDistance;
    /** Set once a widget consumes the tap, so overlapping widgets cannot both fire. */
    private boolean consumed;

    public Ui(Art art, SpriteBatch batch) {
        this.art = art;
        this.batch = batch;
    }

    /** Call once per frame before drawing. */
    public void begin(Viewport viewport) {
        this.viewport = viewport;
        touch.set(Gdx.input.getX(), Gdx.input.getY(), 0f);
        viewport.unproject(touch);
        pointerX = touch.x;
        pointerY = touch.y;
        boolean wasPressed = pressed;
        pressed = Gdx.input.isTouched();
        dragDeltaY = (wasPressed && pressed) ? pointerY - lastPointerY : 0f;
        dragDistance = pressed ? (wasPressed ? dragDistance + Math.abs(dragDeltaY) : 0f) : 0f;
        lastPointerY = pointerY;
        // justTouched fires on press; using it for activation keeps taps responsive on mobile
        // while still being a single, unambiguous event per frame.
        justReleased = Gdx.input.justTouched();
        consumed = false;
    }

    public float pointerX() {
        return pointerX;
    }

    public float pointerY() {
        return pointerY;
    }

    public boolean isPressed() {
        return pressed;
    }

    /** Vertical pointer movement since the previous frame, for flick-scrolling lists. */
    public float dragDeltaY() {
        return dragDeltaY;
    }

    /**
     * True once the current touch has moved far enough to be a scroll rather than a tap. Buttons
     * inside a scrolling list check this so dragging past a row does not also activate it.
     */
    public boolean isScrolling() {
        return dragDistance > 24f;
    }

    public boolean hit(float x, float y, float w, float h) {
        return pointerX >= x && pointerX <= x + w && pointerY >= y && pointerY <= y + h;
    }

    // --- Primitives ----------------------------------------------------------------------

    public void rect(float x, float y, float w, float h, Color color) {
        batch.setColor(color);
        batch.draw(art.pixel, x, y, w, h);
        batch.setColor(Color.WHITE);
    }

    public void rect(float x, float y, float w, float h, Color color, float alpha) {
        batch.setColor(color.r, color.g, color.b, color.a * alpha);
        batch.draw(art.pixel, x, y, w, h);
        batch.setColor(Color.WHITE);
    }

    public void border(float x, float y, float w, float h, float thickness, Color color) {
        rect(x, y, w, thickness, color);
        rect(x, y + h - thickness, w, thickness, color);
        rect(x, y, thickness, h, color);
        rect(x + w - thickness, y, thickness, h, color);
    }

    public void panel(float x, float y, float w, float h) {
        rect(x, y, w, h, Theme.PANEL);
        border(x, y, w, h, 2f, Theme.BORDER);
    }

    /** A labelled progress bar; the workhorse of the fishing HUD. */
    public void bar(float x, float y, float w, float h, float ratio, Color fill, Color background) {
        ratio = clamp01(ratio);
        rect(x, y, w, h, background);
        if (ratio > 0f) rect(x, y, w * ratio, h, fill);
        border(x, y, w, h, 2f, Theme.BORDER);
    }

    /** Draws a tick mark on a bar, used to show the safe-tension threshold. */
    public void barMarker(float x, float y, float w, float h, float ratio, Color color) {
        float mx = x + w * clamp01(ratio);
        rect(mx - 2f, y - 4f, 4f, h + 8f, color);
    }

    // --- Text ----------------------------------------------------------------------------

    public void text(BitmapFont font, String s, float x, float y, Color color) {
        font.setColor(color);
        font.draw(batch, s, x, y);
    }

    public void textCentered(BitmapFont font, String s, float cx, float y, Color color) {
        layout.setText(font, s);
        font.setColor(color);
        font.draw(batch, s, cx - layout.width / 2f, y);
    }

    public void textRight(BitmapFont font, String s, float rightX, float y, Color color) {
        layout.setText(font, s);
        font.setColor(color);
        font.draw(batch, s, rightX - layout.width, y);
    }

    public float textWidth(BitmapFont font, String s) {
        layout.setText(font, s);
        return layout.width;
    }

    /** Wraps text inside a width and returns the height consumed. */
    public float textWrapped(BitmapFont font, String s, float x, float y, float width, Color color) {
        font.setColor(color);
        layout.setText(font, s, color, width, com.badlogic.gdx.utils.Align.left, true);
        font.draw(batch, layout, x, y);
        return layout.height;
    }

    // --- Widgets -------------------------------------------------------------------------

    public boolean button(float x, float y, float w, float h, String label) {
        return button(x, y, w, h, label, true, false);
    }

    public boolean button(float x, float y, float w, float h, String label,
                          boolean enabled, boolean active) {
        boolean over = hit(x, y, w, h);
        Color base = !enabled ? Theme.BUTTON_DISABLED
                : active ? Theme.BUTTON_ACTIVE
                : (over && pressed) ? Theme.BUTTON_DOWN : Theme.BUTTON;

        rect(x, y, w, h, base);
        border(x, y, w, h, 2f, active ? Theme.ACCENT : Theme.BORDER);

        BitmapFont font = art.font;
        layout.setText(font, label);
        if (layout.width > w - 16f) {
            font = art.fontSmall;
            layout.setText(font, label);
        }
        textCentered(font, label, x + w / 2f, y + h / 2f + layout.height / 2f,
                enabled ? Theme.TEXT : Theme.TEXT_DIM);

        if (!enabled || consumed || isScrolling()) return false;
        boolean fired = over && justReleased;
        if (fired) consumed = true;
        return fired;
    }

    /** Hit-tests a region without drawing anything, for widgets that paint themselves. */
    public boolean invisibleButton(float x, float y, float w, float h) {
        if (consumed || isScrolling()) return false;
        boolean fired = hit(x, y, w, h) && justReleased;
        if (fired) consumed = true;
        return fired;
    }

    /**
     * A vertical drag control for manual fishing. Returns the 0..1 value, or {@code current}
     * when the player is not touching it.
     */
    public float verticalSlider(float x, float y, float w, float h, float current, Color fill) {
        rect(x, y, w, h, Theme.PANEL_LIGHT);
        float value = current;
        if (pressed && hit(x - 40f, y, w + 80f, h)) {
            value = clamp01((pointerY - y) / h);
        }
        rect(x, y, w, h * value, fill);
        border(x, y, w, h, 2f, Theme.BORDER);
        return value;
    }

    public Art art() {
        return art;
    }

    public SpriteBatch batch() {
        return batch;
    }

    public static float clamp01(float v) {
        return v < 0f ? 0f : (v > 1f ? 1f : v);
    }

    /** Formats a weight the way an angler would say it. */
    public static String weight(float kg) {
        if (kg < 1f) return Math.round(kg * 1000f) + " g";
        if (kg < 10f) return trim1(kg) + " kg";
        return Math.round(kg) + " kg";
    }

    /** One decimal place without String.format, which is expensive and GWT-unfriendly. */
    public static String trim1(float v) {
        int scaled = Math.round(v * 10f);
        return (scaled / 10) + "." + Math.abs(scaled % 10);
    }

    public static String percent(float ratio) {
        return Math.round(ratio * 100f) + "%";
    }

    /** Thousands separator for gold; keeps big numbers readable on a narrow screen. */
    public static String number(long value) {
        String s = Long.toString(Math.abs(value));
        StringBuilder sb = new StringBuilder();
        int count = 0;
        for (int i = s.length() - 1; i >= 0; i--) {
            sb.append(s.charAt(i));
            if (++count % 3 == 0 && i > 0) sb.append('.');
        }
        if (value < 0) sb.append('-');
        return sb.reverse().toString();
    }

    public static String duration(float seconds) {
        int total = (int) seconds;
        int m = total / 60, s = total % 60;
        return m + ":" + (s < 10 ? "0" : "") + s;
    }
}
