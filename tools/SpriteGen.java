import javax.imageio.ImageIO;
import java.awt.*;
import java.awt.geom.*;
import java.awt.geom.Point2D;
import java.awt.image.BufferedImage;
import java.io.File;
import java.io.PrintWriter;
import java.util.LinkedHashMap;
import java.util.Map;

/**
 * Bakes the game's sprite atlas.
 *
 * <p>Every shape here is drawn from primitives and curves for this project. The reference title's
 * assets are copyrighted and the brief (GDD 2.2) forbids reusing another game's artwork, so none
 * of it is derived from anything else - the silhouettes come from the behaviour each archetype has
 * in the simulation, which is also what makes them readable in play.
 *
 * <p>One atlas rather than loose files: the HTML5 preloader fetches each asset separately, and a
 * dozen extra round-trips is the difference between a fast first load and a slow one.
 *
 * <p>Run: {@code java tools/SpriteGen.java} (output is committed).
 */
public final class SpriteGen {

    private static final int ATLAS = 1024;
    private static final Map<String, int[]> REGIONS = new LinkedHashMap<String, int[]>();

    private static BufferedImage atlas;
    private static Graphics2D g;

    public static void main(String[] args) throws Exception {
        System.setProperty("java.awt.headless", "true");
        atlas = new BufferedImage(ATLAS, ATLAS, BufferedImage.TYPE_INT_ARGB);
        g = atlas.createGraphics();
        g.setRenderingHint(RenderingHints.KEY_ANTIALIASING, RenderingHints.VALUE_ANTIALIAS_ON);
        g.setRenderingHint(RenderingHints.KEY_STROKE_CONTROL, RenderingHints.VALUE_STROKE_PURE);
        g.setRenderingHint(RenderingHints.KEY_RENDERING, RenderingHints.VALUE_RENDER_QUALITY);

        // --- Fish, one per behaviour archetype (GDD 7) -----------------------------------
        // Packed from the project's own licensed source art in art-src/fish, cut off its chroma
        // background by tools/AssetExtract.java. These replace the procedural silhouettes an
        // earlier revision drew here; the archetype each one is assigned to follows its
        // silhouette, since that is the player's fastest read on how a fight will go.
        int fw = 330, fh = 165;
        packFish("fish_runner", 0, 0, fw, fh, "runner.png");
        packFish("fish_power_tank", fw, 0, fw, fh, "power_tank.png");
        packFish("fish_erratic", fw * 2, 0, fw, fh, "erratic.png");
        packFish("fish_diver", 0, fh, fw, fh, "diver.png");
        packFish("fish_trickster", fw, fh, fw, fh, "trickster.png");
        packFish("fish_boss", fw * 2, fh, fw, fh, "boss.png");
        // A distinct sprite for the top rarity tiers, so a legendary catch does not look
        // identical to the common fish that shares its behaviour archetype. Tucked into the gap
        // left by the portrait grid rather than growing the sheet.
        packFish("fish_legendary", 560, 840, fw, fh, "extra_gold.png");

        // --- The player's angler, two poses ---------------------------------------------
        angler("angler_idle", 0, 340, 230, 300, false);
        angler("angler_pull", 230, 340, 230, 300, true);
        boat("boat", 470, 340, 420, 150);

        // --- Team portraits, one per recruitable angler (GDD 8) --------------------------
        String[] ids = {"ag_lam", "ag_hue", "ag_bac_ho", "ag_thao",
                        "ag_kien", "ag_hai_long", "ag_van_nuong", "ag_thien_ngu"};
        // 5 per row at 180px starting at y=650 keeps the second row inside the 1024px atlas;
        // at 190px from y=660 the bottom row was clipped by 16px.
        int ps = 180;
        for (int i = 0; i < ids.length; i++) {
            int col = i % 5, row = i / 5;
            portrait("portrait_" + ids[i], col * ps, 650 + row * ps, ps, ps, i);
        }

        g.dispose();

        new File("assets/sprites").mkdirs();
        ImageIO.write(atlas, "png", new File("assets/sprites/atlas.png"));

        PrintWriter out = new PrintWriter("assets/sprites/atlas.json", "UTF-8");
        out.println("{");
        out.println("  \"size\": " + ATLAS + ",");
        out.println("  \"regions\": {");
        int i = 0;
        for (Map.Entry<String, int[]> e : REGIONS.entrySet()) {
            int[] r = e.getValue();
            out.print("    \"" + e.getKey() + "\": [" + r[0] + ", " + r[1] + ", "
                    + r[2] + ", " + r[3] + "]");
            out.println(++i < REGIONS.size() ? "," : "");
        }
        out.println("  },");
        out.println("  \"anchors\": {");
        i = 0;
        for (Map.Entry<String, float[]> e : ANCHORS.entrySet()) {
            float[] a = e.getValue();
            out.print("    \"" + e.getKey() + "\": ["
                    + round(a[0]) + ", " + round(a[1]) + "]");
            out.println(++i < ANCHORS.size() ? "," : "");
        }
        out.println("  }");
        out.println("}");
        out.close();

        System.out.println("wrote atlas with " + REGIONS.size() + " regions");
    }

    private static String round(float v) {
        return String.valueOf(Math.round(v * 1000) / 1000f);
    }

    /**
     * Scales one source sprite to fit a cell, preserving aspect, and packs it into the atlas.
     *
     * <p>Fit rather than fill: the cells are a fixed grid but the source art varies in
     * proportion, and stretching a fish to the cell would distort exactly the silhouette the
     * archetype is meant to communicate.
     */
    private static void packFish(String name, int ox, int oy, int w, int h, String file) {
        mark(name, ox, oy, w, h);
        File source = new File("art-src/fish", file);
        if (!source.isFile()) {
            throw new IllegalStateException("Missing source art: " + source);
        }
        BufferedImage src;
        try {
            src = ImageIO.read(source);
        } catch (Exception e) {
            throw new IllegalStateException("Cannot read " + source, e);
        }

        double scale = Math.min((w - 8.0) / src.getWidth(), (h - 8.0) / src.getHeight());
        int dw = (int) Math.round(src.getWidth() * scale);
        int dh = (int) Math.round(src.getHeight() * scale);
        Graphics2D c = (Graphics2D) g.create();
        c.setRenderingHint(RenderingHints.KEY_INTERPOLATION,
                RenderingHints.VALUE_INTERPOLATION_BICUBIC);
        c.drawImage(src, ox + (w - dw) / 2, oy + (h - dh) / 2, dw, dh, null);
        c.dispose();
    }

    private static void mark(String name, int x, int y, int w, int h) {
        REGIONS.put(name, new int[]{x, y, w, h});
    }

    // =====================================================================================
    // Fish
    // =====================================================================================

    private static Shape triangle(float x1, float y1, float x2, float y2, float x3, float y3) {
        GeneralPath p = new GeneralPath();
        p.moveTo(x1, y1);
        p.lineTo(x2, y2);
        p.lineTo(x3, y3);
        p.closePath();
        return p;
    }

    // =====================================================================================
    // The player's angler
    // =====================================================================================

    private static final Color SKIN = new Color(0xE8B48C);
    private static final Color SKIN_DARK = new Color(0xC98F68);
    private static final Color HAT = new Color(0xD9C08A);
    private static final Color SHIRT = new Color(0x3E7FA8);
    private static final Color SHIRT_DARK = new Color(0x2C5F82);
    private static final Color TROUSERS = new Color(0x35455C);

    /** Grip point of the rod hand, in 0..1 sprite coordinates, per pose. */
    private static final Map<String, float[]> ANCHORS = new LinkedHashMap<String, float[]>();

    /**
     * Side-view angler facing right, towards the water. The pulling pose leans back with the rod
     * hand high, so the state of the fight is readable from the character before the gauges.
     *
     * <p>The rod is deliberately not baked in: it is drawn at runtime as a curve so it can bend
     * with line tension. The hand position is exported to the atlas as an anchor so the renderer
     * knows where to start that curve.
     */
    private static void angler(String name, int ox, int oy, int w, int h, boolean pulling) {
        mark(name, ox, oy, w, h);

        BufferedImage buffer = new BufferedImage(w, h, BufferedImage.TYPE_INT_ARGB);
        Graphics2D c = buffer.createGraphics();
        c.setRenderingHint(RenderingHints.KEY_ANTIALIASING, RenderingHints.VALUE_ANTIALIAS_ON);
        c.setRenderingHint(RenderingHints.KEY_STROKE_CONTROL, RenderingHints.VALUE_STROKE_PURE);

        AffineTransform saved = c.getTransform();
        c.rotate(Math.toRadians(pulling ? -11 : 0), w * 0.5f, h * 0.92f);

        // Chibi proportions - head roughly a third of the figure. At the size this is drawn on a
        // phone (about 130px tall inside a busy scene) realistic proportions read as a stick;
        // a large head and a strong hat silhouette stay legible.
        float cx = w * 0.46f;
        float headCy = h * 0.27f, headR = h * 0.135f;
        float shoulderY = h * 0.46f, hipY = h * 0.68f;

        // Legs.
        c.setColor(TROUSERS);
        c.setStroke(new BasicStroke(w * 0.115f, BasicStroke.CAP_ROUND, BasicStroke.JOIN_ROUND));
        c.draw(new Line2D.Float(cx - w * 0.02f, hipY, cx - w * 0.10f, h * 0.90f));
        c.draw(new Line2D.Float(cx + w * 0.02f, hipY, cx + w * 0.14f, h * 0.90f));

        c.setColor(new Color(0x22303F));
        c.setStroke(new BasicStroke(w * 0.075f, BasicStroke.CAP_ROUND, BasicStroke.JOIN_ROUND));
        c.draw(new Line2D.Float(cx - w * 0.13f, h * 0.925f, cx - w * 0.03f, h * 0.925f));
        c.draw(new Line2D.Float(cx + w * 0.12f, h * 0.925f, cx + w * 0.24f, h * 0.925f));

        // Rear arm, behind the torso and in sleeve colour for its whole length: a skin-toned arm
        // crossing a blue shirt read as a slab.
        c.setColor(SHIRT_DARK);
        c.setStroke(new BasicStroke(w * 0.062f, BasicStroke.CAP_ROUND, BasicStroke.JOIN_ROUND));
        GeneralPath rear = new GeneralPath();
        rear.moveTo(cx - w * 0.07f, shoulderY + h * 0.02f);
        if (pulling) {
            rear.quadTo(cx - w * 0.19f, shoulderY + h * 0.07f, cx - w * 0.10f, shoulderY - h * 0.01f);
        } else {
            rear.quadTo(cx - w * 0.15f, shoulderY + h * 0.10f, cx - w * 0.12f, shoulderY + h * 0.19f);
        }
        c.draw(rear);

        // Torso.
        c.setColor(SHIRT);
        GeneralPath torso = new GeneralPath();
        torso.moveTo(cx - w * 0.135f, shoulderY);
        torso.curveTo(cx - w * 0.17f, shoulderY + h * 0.12f, cx - w * 0.12f, hipY, cx - w * 0.09f, hipY + h * 0.02f);
        torso.lineTo(cx + w * 0.12f, hipY + h * 0.02f);
        torso.curveTo(cx + w * 0.16f, hipY, cx + w * 0.155f, shoulderY + h * 0.06f, cx + w * 0.125f, shoulderY);
        torso.closePath();
        c.fill(torso);
        c.setColor(SHIRT_DARK);
        c.fill(new Rectangle2D.Float(cx - w * 0.13f, hipY - h * 0.045f, w * 0.25f, h * 0.045f));

        // Head.
        c.setColor(SKIN);
        c.fill(new Ellipse2D.Float(cx - headR, headCy - headR, headR * 2f, headR * 2f));
        // No separate neck shape: at this size it sat under the chin and read as a beard.

        // Nón lá: a cone, not a round brim. It is the strongest silhouette cue available and it
        // places the character without needing any detail that survives at this size.
        c.setColor(HAT);
        // Sized to sit on top of the head, not over it. A first pass ran the cone from well
        // above the head down past the chin and buried the whole face.
        float apexY = headCy - headR * 1.30f;
        float brimY = headCy - headR * 0.10f;
        float brimHalf = w * 0.225f;
        GeneralPath hat = new GeneralPath();
        hat.moveTo(cx, apexY);
        hat.lineTo(cx + brimHalf, brimY);
        hat.curveTo(cx + brimHalf * 0.45f, brimY + headR * 0.30f,
                    cx - brimHalf * 0.45f, brimY + headR * 0.30f, cx - brimHalf, brimY);
        hat.closePath();
        c.fill(hat);
        c.setColor(new Color(0xB89E68));
        c.setStroke(new BasicStroke(Math.max(1.4f, w * 0.009f)));
        for (int i = 1; i <= 3; i++) {
            float t = i / 4f;
            float y = apexY + (brimY - apexY) * t;
            c.draw(new Line2D.Float(cx - brimHalf * t, y, cx + brimHalf * t, y));
        }

        // Eyes, offset towards the direction the angler faces so the head reads as a 3/4 view.
        c.setColor(new Color(0x2A2A33));
        float eyeY = headCy + headR * 0.22f;
        c.fill(new Ellipse2D.Float(cx - w * 0.005f, eyeY, w * 0.036f, h * 0.030f));
        c.fill(new Ellipse2D.Float(cx + w * 0.062f, eyeY, w * 0.036f, h * 0.030f));

        // Front arm: sleeve to the elbow, bare forearm to the grip.
        float gripX, gripY, elbowX, elbowY;
        if (pulling) {
            elbowX = cx + w * 0.18f; elbowY = shoulderY + h * 0.05f;
            gripX  = cx + w * 0.25f; gripY  = shoulderY - h * 0.08f;
        } else {
            elbowX = cx + w * 0.18f; elbowY = shoulderY + h * 0.10f;
            gripX  = cx + w * 0.30f; gripY  = shoulderY + h * 0.03f;
        }
        c.setColor(SHIRT);
        c.setStroke(new BasicStroke(w * 0.070f, BasicStroke.CAP_ROUND, BasicStroke.JOIN_ROUND));
        c.draw(new Line2D.Float(cx + w * 0.06f, shoulderY + h * 0.02f, elbowX, elbowY));
        c.setColor(SKIN);
        c.setStroke(new BasicStroke(w * 0.058f, BasicStroke.CAP_ROUND, BasicStroke.JOIN_ROUND));
        c.draw(new Line2D.Float(elbowX, elbowY, gripX, gripY));

        // Grip in 0..1 sprite space. Reading it back off the Graphics2D transform would include
        // the translation into the sprite's atlas slot, so the pose rotation is applied by hand.
        double theta = Math.toRadians(pulling ? -11 : 0);
        double px = w * 0.5f, py = h * 0.92f;
        double dx = gripX - px, dy = gripY - py;
        double rx = px + dx * Math.cos(theta) - dy * Math.sin(theta);
        double ry = py + dx * Math.sin(theta) + dy * Math.cos(theta);
        ANCHORS.put(name + "_grip", new float[]{(float) (rx / w), (float) (ry / h)});

        c.setTransform(saved);

        // Light from the upper right, matching the fish shading, so the scene holds together.
        c.setComposite(AlphaComposite.SrcAtop);
        c.setPaint(new GradientPaint(0, 0, new Color(0, 0, 0, 0), w, h, new Color(0, 0, 0, 90)));
        c.fillRect(0, 0, w, h);
        c.setComposite(AlphaComposite.SrcOver);
        c.dispose();

        g.drawImage(buffer, ox, oy, null);
    }

    /** Small boat the angler stands in. */
    private static void boat(String name, int ox, int oy, int w, int h) {
        mark(name, ox, oy, w, h);
        Graphics2D c = (Graphics2D) g.create(ox, oy, w, h);
        c.setRenderingHint(RenderingHints.KEY_ANTIALIASING, RenderingHints.VALUE_ANTIALIAS_ON);

        GeneralPath hull = new GeneralPath();
        hull.moveTo(w * 0.02f, h * 0.30f);
        hull.curveTo(w * 0.16f, h * 0.95f, w * 0.84f, h * 0.95f, w * 0.98f, h * 0.30f);
        hull.closePath();
        c.setColor(new Color(0x6B4A32));
        c.fill(hull);

        c.setColor(new Color(0x8A6142));
        c.fill(new RoundRectangle2D.Float(0, h * 0.20f, w, h * 0.16f, h * 0.16f, h * 0.16f));
        c.setColor(new Color(0x4A3222));
        c.fill(new Rectangle2D.Float(w * 0.10f, h * 0.44f, w * 0.80f, h * 0.05f));
        c.dispose();
    }

    // =====================================================================================
    // Team portraits
    // =====================================================================================

    private static final Color[] HAIR = {
            new Color(0x2B2118), new Color(0x1C1C22), new Color(0x8A8A93),
            new Color(0x3D2A1E), new Color(0x24303D), new Color(0x4A2B2B),
            new Color(0x2E3B46), new Color(0xB9C6D1),
    };
    private static final Color[] ROBE = {
            new Color(0x3E7FA8), new Color(0x9C5B7A), new Color(0x6B7F4A),
            new Color(0x8A6142), new Color(0x4E6B8A), new Color(0xA85C3E),
            new Color(0x6A5A9C), new Color(0x2F8C86),
    };

    /**
     * Bust portrait. The eight anglers are distinguished by hair silhouette, palette and a role
     * accessory rather than by facial detail, which does not survive being drawn at 90px on a
     * phone.
     */
    private static void portrait(String name, int ox, int oy, int size, int unused, int index) {
        mark(name, ox, oy, size, size);
        Graphics2D c = (Graphics2D) g.create(ox, oy, size, size);
        c.setRenderingHint(RenderingHints.KEY_ANTIALIASING, RenderingHints.VALUE_ANTIALIAS_ON);

        float s = size;
        Color hair = HAIR[index % HAIR.length];
        Color robe = ROBE[index % ROBE.length];
        boolean longHair = index == 1 || index == 4 || index == 6 || index == 7;
        boolean hasHat = index == 2 || index == 5;

        // Background disc, so a portrait reads as a unit against any panel colour.
        c.setColor(new Color(robe.getRed(), robe.getGreen(), robe.getBlue(), 70));
        c.fill(new Ellipse2D.Float(s * 0.06f, s * 0.06f, s * 0.88f, s * 0.88f));

        // Shoulders.
        c.setColor(robe);
        c.fill(new Arc2D.Float(s * 0.10f, s * 0.60f, s * 0.80f, s * 0.70f, 0, 180, Arc2D.PIE));
        c.setColor(robe.darker());
        c.fill(triangle(s * 0.50f, s * 0.62f, s * 0.40f, s * 0.95f, s * 0.60f, s * 0.95f));

        // Long hair falls past the jaw on both sides. Drawn as a disc behind the head it read
        // as a hood, because the face covered the middle and only a ring stayed visible.
        if (longHair) {
            c.setColor(hair);
            GeneralPath fall = new GeneralPath();
            fall.moveTo(s * 0.30f, s * 0.34f);
            fall.curveTo(s * 0.20f, s * 0.52f, s * 0.20f, s * 0.68f, s * 0.24f, s * 0.80f);
            fall.lineTo(s * 0.38f, s * 0.78f);
            fall.curveTo(s * 0.32f, s * 0.62f, s * 0.32f, s * 0.48f, s * 0.36f, s * 0.36f);
            fall.closePath();
            c.fill(fall);
            GeneralPath fall2 = new GeneralPath();
            fall2.moveTo(s * 0.70f, s * 0.34f);
            fall2.curveTo(s * 0.80f, s * 0.52f, s * 0.80f, s * 0.68f, s * 0.76f, s * 0.80f);
            fall2.lineTo(s * 0.62f, s * 0.78f);
            fall2.curveTo(s * 0.68f, s * 0.62f, s * 0.68f, s * 0.48f, s * 0.64f, s * 0.36f);
            fall2.closePath();
            c.fill(fall2);
        }

        // Face. No separate neck shape: at this size it overlapped the jaw and read as a mouth.
        c.setColor(SKIN);
        c.fill(new Ellipse2D.Float(s * 0.30f, s * 0.24f, s * 0.40f, s * 0.46f));

        // Hair front.
        c.setColor(hair);
        GeneralPath fringe = new GeneralPath();
        fringe.moveTo(s * 0.28f, s * 0.46f);
        fringe.curveTo(s * 0.26f, s * 0.20f, s * 0.74f, s * 0.20f, s * 0.72f, s * 0.46f);
        fringe.curveTo(s * 0.66f, s * 0.34f, s * 0.34f, s * 0.34f, s * 0.28f, s * 0.46f);
        fringe.closePath();
        c.fill(fringe);

        // Eyes.
        c.setColor(new Color(0x2A2A33));
        c.fill(new Ellipse2D.Float(s * 0.39f, s * 0.44f, s * 0.055f, s * 0.075f));
        c.fill(new Ellipse2D.Float(s * 0.555f, s * 0.44f, s * 0.055f, s * 0.075f));

        if (hasHat) {
            c.setColor(HAT);
            c.fill(new Ellipse2D.Float(s * 0.16f, s * 0.28f, s * 0.68f, s * 0.09f));
            GeneralPath crown = new GeneralPath();
            crown.moveTo(s * 0.32f, s * 0.31f);
            crown.curveTo(s * 0.34f, s * 0.12f, s * 0.66f, s * 0.12f, s * 0.68f, s * 0.31f);
            crown.closePath();
            c.fill(crown);
        }

        // Role accessory, in the same order as the roster.
        c.setColor(new Color(0xF4C93C));
        switch (index % 5) {
            case 0: // Striker: shoulder guard
                c.fill(new Arc2D.Float(s * 0.62f, s * 0.66f, s * 0.26f, s * 0.26f, 20, 140, Arc2D.PIE));
                break;
            case 1: // Controller: collar band
                c.fill(new Rectangle2D.Float(s * 0.36f, s * 0.70f, s * 0.28f, s * 0.045f));
                break;
            case 2: // Anchor: rope coil
                c.setStroke(new BasicStroke(s * 0.035f));
                c.draw(new Ellipse2D.Float(s * 0.14f, s * 0.70f, s * 0.16f, s * 0.16f));
                break;
            case 3: // Hunter: feather
                c.fill(triangle(s * 0.74f, s * 0.30f, s * 0.90f, s * 0.10f, s * 0.80f, s * 0.34f));
                break;
            default: // Support: satchel strap
                c.fill(new Rectangle2D.Float(s * 0.30f, s * 0.72f, s * 0.40f, s * 0.035f));
                break;
        }
        c.dispose();
    }
}
