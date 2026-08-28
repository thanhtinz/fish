import javax.imageio.ImageIO;
import java.awt.*;
import java.awt.geom.*;
import java.awt.image.BufferedImage;
import java.io.File;

/**
 * Generates the Android launcher icons and the web logo.
 *
 * <p>Original mark drawn from primitives, for the same reason the in-game art is procedural: the
 * reference material for this project is third-party artwork of unknown provenance and must not be
 * reused. Run: {@code java tools/IconGen.java} (output is committed).
 */
public final class IconGen {

    private static final int[] DENSITIES = {48, 72, 96, 144, 192};
    private static final String[] BUCKETS =
            {"mdpi", "hdpi", "xhdpi", "xxhdpi", "xxxhdpi"};

    public static void main(String[] args) throws Exception {
        System.setProperty("java.awt.headless", "true");
        for (int i = 0; i < DENSITIES.length; i++) {
            File dir = new File("android/res/mipmap-" + BUCKETS[i]);
            dir.mkdirs();
            ImageIO.write(icon(DENSITIES[i]), "png", new File(dir, "ic_launcher.png"));
        }
        File web = new File("html/webapp");
        web.mkdirs();
        ImageIO.write(icon(512), "png", new File(web, "logo.png"));
        System.out.println("wrote launcher icons and web logo");
    }

    /** A hook over a deep-water gradient, with a fish silhouette caught on it. */
    private static BufferedImage icon(int size) {
        BufferedImage image = new BufferedImage(size, size, BufferedImage.TYPE_INT_ARGB);
        Graphics2D g = image.createGraphics();
        g.setRenderingHint(RenderingHints.KEY_ANTIALIASING, RenderingHints.VALUE_ANTIALIAS_ON);
        g.setRenderingHint(RenderingHints.KEY_STROKE_CONTROL, RenderingHints.VALUE_STROKE_PURE);

        float s = size / 192f;

        // Rounded background with the game's water gradient.
        g.setPaint(new GradientPaint(0, 0, new Color(0x1B4B63),
                0, size, new Color(0x061626)));
        g.fill(new RoundRectangle2D.Float(0, 0, size, size, size * 0.24f, size * 0.24f));

        // Surface line.
        g.setColor(new Color(0x40C7EB));
        g.setStroke(new BasicStroke(3f * s, BasicStroke.CAP_ROUND, BasicStroke.JOIN_ROUND));
        GeneralPath surface = new GeneralPath();
        surface.moveTo(14 * s, 52 * s);
        for (int x = 14; x <= 178; x += 8) {
            surface.lineTo(x * s, (52 + Math.sin(x * 0.16) * 4) * s);
        }
        g.draw(surface);

        // Line down from the surface.
        g.setColor(new Color(0xBFE8F5));
        g.setStroke(new BasicStroke(2.4f * s));
        g.draw(new Line2D.Float(96 * s, 50 * s, 96 * s, 108 * s));

        // Hook.
        g.setStroke(new BasicStroke(6f * s, BasicStroke.CAP_ROUND, BasicStroke.JOIN_ROUND));
        g.setColor(new Color(0xF4C93C));
        g.draw(new Arc2D.Float(78 * s, 100 * s, 36 * s, 40 * s, 200, 210, Arc2D.OPEN));

        // Fish silhouette.
        g.setColor(new Color(0x8FE3C8));
        GeneralPath fish = new GeneralPath();
        fish.append(new Ellipse2D.Float(96 * s, 118 * s, 62 * s, 32 * s), false);
        fish.moveTo(98 * s, 134 * s);
        fish.lineTo(70 * s, 118 * s);
        fish.lineTo(70 * s, 150 * s);
        fish.closePath();
        g.fill(fish);

        g.setColor(new Color(0x061626));
        g.fill(new Ellipse2D.Float(142 * s, 128 * s, 7 * s, 7 * s));

        g.dispose();
        return image;
    }
}
