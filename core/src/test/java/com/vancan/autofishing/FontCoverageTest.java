package com.vancan.autofishing;

import com.vancan.autofishing.auto.AutoStrategy;
import com.vancan.autofishing.content.AnglerDef;
import com.vancan.autofishing.content.GameContent;
import com.vancan.autofishing.content.GearSlot;
import com.vancan.autofishing.content.GearTemplate;
import com.vancan.autofishing.content.SpotDef;
import com.vancan.autofishing.content.TeamRole;
import com.vancan.autofishing.sim.Archetype;
import com.vancan.autofishing.sim.FailureReason;
import com.vancan.autofishing.sim.FishPhase;
import com.vancan.autofishing.sim.Rarity;
import com.vancan.autofishing.sim.SkillDef;
import com.vancan.autofishing.sim.SpeciesDef;
import org.junit.jupiter.api.Test;

import java.io.File;
import java.io.IOException;
import java.nio.charset.StandardCharsets;
import java.nio.file.Files;
import java.util.HashSet;
import java.util.Set;
import java.util.TreeSet;

import static org.junit.jupiter.api.Assertions.assertTrue;

/**
 * The bitmap font ships a fixed glyph set, and a character it does not contain renders as
 * <em>nothing</em> - no error, no fallback box, just a hole in the text. That failure mode is
 * silent and would ship, so every user-visible string in the content tables is checked against the
 * atlas here.
 *
 * <p>If this fails after adding content, add the missing characters to the charset in
 * {@code tools/FontGen.java} and re-run it.
 */
class FontCoverageTest {

    @Test
    void everyAuthoredStringCanBeRendered() {
        Set<Character> glyphs = loadGlyphs();
        Set<Character> missing = new TreeSet<Character>();
        GameContent c = TestContent.get();

        for (SpeciesDef s : c.species.values()) {
            check(s.name, glyphs, missing);
            check(s.description, glyphs, missing);
        }
        for (SpotDef s : c.spots.values()) {
            check(s.name, glyphs, missing);
            check(s.theme, glyphs, missing);
            check(s.description, glyphs, missing);
        }
        for (GearTemplate g : c.gear.values()) {
            check(g.name, glyphs, missing);
            check(g.description, glyphs, missing);
        }
        for (AnglerDef a : c.anglers.values()) {
            check(a.name, glyphs, missing);
            check(a.description, glyphs, missing);
        }
        for (SkillDef s : c.skills.values()) {
            check(s.name, glyphs, missing);
            check(s.description, glyphs, missing);
        }

        // Enum display names are authored text too and reach the screen the same way.
        for (Rarity r : Rarity.values()) check(r.displayName, glyphs, missing);
        for (Archetype a : Archetype.values()) check(a.displayName, glyphs, missing);
        for (FishPhase p : FishPhase.values()) check(p.displayName, glyphs, missing);
        for (FailureReason f : FailureReason.values()) check(f.displayName, glyphs, missing);
        for (GearSlot g : GearSlot.values()) check(g.displayName, glyphs, missing);
        for (TeamRole t : TeamRole.values()) {
            check(t.displayName, glyphs, missing);
            check(t.summary, glyphs, missing);
        }
        for (AutoStrategy s : AutoStrategy.values()) {
            check(s.displayName, glyphs, missing);
            check(s.summary, glyphs, missing);
            check(s.description, glyphs, missing);
        }

        // Symbols the HUD composes at runtime rather than reading from content.
        check("⧫◆★♦·—…", glyphs, missing);

        assertTrue(missing.isEmpty(),
                "The shipped font has no glyph for: " + describe(missing)
                        + "\nThese would render as invisible gaps. Add them to the charset in "
                        + "tools/FontGen.java and re-run it.");
    }

    private void check(String text, Set<Character> glyphs, Set<Character> missing) {
        if (text == null) return;
        for (int i = 0; i < text.length(); i++) {
            char ch = text.charAt(i);
            if (ch == '\n' || ch == '\r' || ch == '\t') continue;
            if (!glyphs.contains(Character.valueOf(ch))) missing.add(Character.valueOf(ch));
        }
    }

    private String describe(Set<Character> missing) {
        StringBuilder sb = new StringBuilder();
        for (Character ch : missing) {
            if (sb.length() > 0) sb.append(", ");
            sb.append('\'').append(ch.charValue()).append("' (U+")
              .append(Integer.toHexString(ch.charValue()).toUpperCase()).append(')');
        }
        return sb.toString();
    }

    /** Parses the char ids out of the AngelCode .fnt the game ships. */
    private Set<Character> loadGlyphs() {
        File fnt = fontFile();
        Set<Character> glyphs = new HashSet<Character>();
        try {
            for (String line : Files.readAllLines(fnt.toPath(), StandardCharsets.UTF_8)) {
                if (!line.startsWith("char id=")) continue;
                int start = "char id=".length();
                int end = line.indexOf(' ', start);
                glyphs.add(Character.valueOf(
                        (char) Integer.parseInt(line.substring(start, end).trim())));
            }
        } catch (IOException e) {
            throw new IllegalStateException("Cannot read " + fnt, e);
        }
        glyphs.add(Character.valueOf(' '));
        return glyphs;
    }

    private File fontFile() {
        String dataRoot = System.getProperty("vancan.dataRoot");
        if (dataRoot != null) {
            File candidate = new File(new File(dataRoot).getParentFile(), "fonts/game.fnt");
            if (candidate.isFile()) return candidate;
        }
        File dir = new File("").getAbsoluteFile();
        for (int i = 0; i < 5 && dir != null; i++) {
            File candidate = new File(dir, "assets/fonts/game.fnt");
            if (candidate.isFile()) return candidate;
            dir = dir.getParentFile();
        }
        throw new IllegalStateException("Could not locate assets/fonts/game.fnt");
    }
}
