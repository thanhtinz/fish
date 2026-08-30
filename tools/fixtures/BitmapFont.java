/**
 * Stands in for the font class a J2ME game ships, so that switching one to the device font can be
 * proved against a real JVM.
 *
 * <p>A real one takes a {@code javax.microedition.lcdui.Graphics}, and no desktop JVM has that
 * class - so a proof that could only target MIDP could never be run through a verifier at all.
 * This has the same <em>shape</em>: a surface, a string, and the numbers to place it at, plus the
 * measuring methods that go with it. Everything the rewrite decides - which local slot each
 * argument is in, how many of them the delegate takes, what the stack depth ends up being - is
 * decided from that shape, so proving it here proves it for the real one.
 *
 * <p>Compiled by tools/make-fixtures.sh; used by tools/verify-roundtrip.sh.
 */
public class BitmapFont {

    /** Blits each character out of a glyph sheet. Vietnamese comes out blank, which is the point. */
    public void drawString(java.io.PrintStream surface, String text, int x, int y) {
        surface.println("[sheet]" + text + "@" + x + "," + y);
    }

    /** Widths measured from the sheet's own cells. */
    public int stringWidth(String text) {
        return text.length() * 7;
    }

    public int getHeight() {
        return 12;
    }

    public static void main(String[] args) {
        BitmapFont font = new BitmapFont();
        font.drawString(System.out, "Thoát", 3, 4);
        System.out.println(font.stringWidth("Thoát"));
        System.out.println(font.getHeight());
    }
}
