package com.vancan.autofishing.screen;

import com.badlogic.gdx.math.Rectangle;
import com.badlogic.gdx.scenes.scene2d.utils.ScissorStack;
import com.vancan.autofishing.VanCanGame;
import com.vancan.autofishing.ui.Theme;

/** A {@link BaseScreen} whose content area scrolls vertically by dragging. */
public abstract class ScrollScreen extends BaseScreen {

    private float scrollY;
    private float contentLength;
    private final Rectangle scissorArea = new Rectangle();
    private final Rectangle scissor = new Rectangle();

    protected ScrollScreen(VanCanGame game) {
        super(game);
    }

    /** Row layout origin: subtract from this as rows are laid out downwards. */
    protected float cursorY() {
        return contentTop() - Theme.PAD + scrollY;
    }

    /** Called by subclasses once they know how tall their content is. */
    protected void setContentLength(float length) {
        contentLength = length;
    }

    @Override
    protected void update(float delta) {
        if (ui.isPressed() && ui.hit(0, contentBottom(), Theme.WORLD_WIDTH, contentHeight())) {
            scrollY -= ui.dragDeltaY();
        }
        float max = Math.max(0f, contentLength - contentHeight() + Theme.PAD * 2f);
        if (scrollY < 0f) scrollY = 0f;
        if (scrollY > max) scrollY = max;
    }

    @Override
    protected final void drawContent(float delta) {
        // Clip to the content area so rows do not bleed under the header and nav bar.
        game.batch.flush();
        scissorArea.set(0, contentBottom(), Theme.WORLD_WIDTH, contentHeight());
        ScissorStack.calculateScissors(game.viewport.getCamera(),
                game.viewport.getScreenX(), game.viewport.getScreenY(),
                game.viewport.getScreenWidth(), game.viewport.getScreenHeight(),
                game.batch.getTransformMatrix(), scissorArea, scissor);
        boolean pushed = ScissorStack.pushScissors(scissor);
        try {
            drawRows(delta);
        } finally {
            if (pushed) {
                game.batch.flush();
                ScissorStack.popScissors();
            }
        }
        drawScrollHint();
    }

    private void drawScrollHint() {
        float max = Math.max(0f, contentLength - contentHeight());
        if (max <= 1f) return;
        float trackH = contentHeight() - Theme.PAD * 2f;
        float thumbH = Math.max(60f, trackH * (contentHeight() / contentLength));
        float t = scrollY / max;
        float y = contentTop() - Theme.PAD - thumbH - t * (trackH - thumbH);
        ui.rect(Theme.WORLD_WIDTH - 12f, y, 6f, thumbH, Theme.BORDER, 0.7f);
    }

    protected abstract void drawRows(float delta);
}
