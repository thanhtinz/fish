import javax.imageio.ImageIO;
import java.awt.*;
import java.awt.font.FontRenderContext;
import java.awt.font.GlyphVector;
import java.awt.geom.Rectangle2D;
import java.awt.image.BufferedImage;
import java.io.File;
import java.io.PrintWriter;
import java.util.ArrayList;
import java.util.List;

/**
 * Generates the AngelCode bitmap font the game ships with.
 *
 * <p>libGDX's built-in font is ASCII only, and the UI is in Vietnamese. The usual answer -
 * gdx-freetype rasterising a TTF at runtime - is not available here: FreeType is a native library
 * and does not exist on the GWT/HTML5 backend, which is one of the four targets. A pre-baked
 * bitmap font is the only form that works identically on desktop, web, Android and iOS.
 *
 * <p>Run: {@code java tools/FontGen.java} (output is committed; this is a one-off dev tool).
 * Source face is DejaVu Sans, which covers the full Vietnamese range and is redistributable.
 */
public final class FontGen {

    private static final String FONT_PATH = "/usr/share/fonts/truetype/dejavu/DejaVuSans-Bold.ttf";
    private static final int PAGE = 512;

    public static void main(String[] args) throws Exception {
        System.setProperty("java.awt.headless", "true");
        generate(32, "assets/fonts/game.fnt", "game.png", 512);
    }

    private static String charset() {
        StringBuilder sb = new StringBuilder();
        for (char c = 32; c <= 126; c++) sb.append(c);
        // Vietnamese: base vowels with every tone/diacritic combination, plus d-stroke.
        sb.append("ÀÁÂÃÈÉÊÌÍÒÓÔÕÙÚÝàáâãèéêìíòóôõùúýĂăĐđĨĩŨũƠơƯư");
        sb.append("ẠạẢảẤấẦầẨẩẪẫẬậẮắẰằẲẳẴẵẶặẸẹẺẻẼẽẾếỀềỂểỄễỆệ");
        sb.append("ỈỉỊịỌọỎỏỐốỒồỔổỖỗỘộỚớỜờỞởỠỡỢợỤụỦủỨứỪừỬửỮữỰự");
        sb.append("ỲỳỴỵỶỷỸỹ");
        sb.append("×÷°±·→←↑↓★☆♦◆⧫●○■□▲▼✓✗–—…«»‹›“”‘’€₫");
        return sb.toString();
    }

    private static void generate(int size, String fntPath, String pngName, int pageSize)
            throws Exception {
        Font font = Font.createFont(Font.TRUETYPE_FONT, new File(FONT_PATH))
                .deriveFont(Font.PLAIN, (float) size);

        BufferedImage image = new BufferedImage(pageSize, pageSize, BufferedImage.TYPE_INT_ARGB);
        Graphics2D g = image.createGraphics();
        g.setRenderingHint(RenderingHints.KEY_TEXT_ANTIALIASING,
                RenderingHints.VALUE_TEXT_ANTIALIAS_ON);
        g.setRenderingHint(RenderingHints.KEY_ANTIALIASING, RenderingHints.VALUE_ANTIALIAS_ON);
        g.setFont(font);
        g.setColor(Color.WHITE);

        FontMetrics metrics = g.getFontMetrics();
        FontRenderContext frc = g.getFontRenderContext();
        int lineHeight = metrics.getHeight();
        int base = metrics.getAscent();

        String chars = charset();
        List<String> lines = new ArrayList<String>();
        int x = 1, y = 1, rowHeight = 0;

        for (int i = 0; i < chars.length(); i++) {
            char c = chars.charAt(i);
            if (!font.canDisplay(c)) {
                System.out.println("WARNING: face cannot display U+"
                        + Integer.toHexString(c) + " - skipped");
                continue;
            }
            String s = String.valueOf(c);
            GlyphVector gv = font.createGlyphVector(frc, s);
            Rectangle2D bounds = gv.getVisualBounds();

            int w = (int) Math.ceil(bounds.getWidth()) + 2;
            int h = (int) Math.ceil(bounds.getHeight()) + 2;
            if (w <= 2) w = Math.max(2, metrics.charWidth(c));
            if (h <= 2) h = 2;

            if (x + w + 1 > pageSize) {
                x = 1;
                y += rowHeight + 1;
                rowHeight = 0;
            }
            if (y + h + 1 > pageSize) {
                throw new IllegalStateException("Glyphs do not fit in a " + pageSize
                        + "px page; raise the page size or lower the font size.");
            }

            int drawX = x - (int) Math.floor(bounds.getX()) + 1;
            int drawY = y - (int) Math.floor(bounds.getY()) + 1;
            g.drawString(s, drawX, drawY);

            int xoffset = (int) Math.floor(bounds.getX()) - 1;
            int yoffset = base + (int) Math.floor(bounds.getY()) - 1;
            lines.add("char id=" + (int) c + " x=" + x + " y=" + y + " width=" + w
                    + " height=" + h + " xoffset=" + xoffset + " yoffset=" + yoffset
                    + " xadvance=" + metrics.charWidth(c) + " page=0 chnl=15");

            x += w + 1;
            rowHeight = Math.max(rowHeight, h);
        }
        g.dispose();

        File png = new File(new File(fntPath).getParentFile(), pngName);
        ImageIO.write(image, "png", png);

        PrintWriter out = new PrintWriter(fntPath, "UTF-8");
        out.println("info face=\"DejaVuSans\" size=" + size + " bold=1 italic=0 charset=\"\" "
                + "unicode=1 stretchH=100 smooth=1 aa=1 padding=0,0,0,0 spacing=1,1");
        out.println("common lineHeight=" + lineHeight + " base=" + base + " scaleW=" + pageSize
                + " scaleH=" + pageSize + " pages=1 packed=0");
        out.println("page id=0 file=\"" + pngName + "\"");
        out.println("chars count=" + lines.size());
        for (String line : lines) out.println(line);
        out.close();

        System.out.println("wrote " + fntPath + " and " + png + " (" + lines.size() + " glyphs)");
    }
}
