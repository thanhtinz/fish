/**
 * Fixture used to generate a realistic class file for the localizer's tests.
 *
 * <p>Compiled once by tools/make-fixtures.sh and the resulting .class is committed, so the Rust
 * test suite stays hermetic and does not need a JDK. Regenerate it only when the fixture needs to
 * cover a new case.
 *
 * <p>The strings below are deliberately varied: display text, a format placeholder, a very short
 * label, a resource path that must NOT be treated as translatable, and text that is already
 * non-ASCII.
 */
public class SampleGame {

    private static final String TITLE = "Dragon Quest Online";
    private static final String MENU_START = "Start Game";
    private static final String MENU_QUIT = "Quit";
    private static final String NPC_LINE = "You have arrived at last, traveller.";
    private static final String STATUS = "HP: %d / %d";
    private static final String RESOURCE = "/img/hud.png";
    private static final String CJK = "装备";

    public static void main(String[] args) {
        System.out.println(TITLE);
        System.out.println(MENU_START);
        System.out.println(MENU_QUIT);
        System.out.println(NPC_LINE);
        System.out.println(STATUS);
        System.out.println(RESOURCE);
        System.out.println(CJK);
    }
}
