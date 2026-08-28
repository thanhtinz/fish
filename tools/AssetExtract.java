import javax.imageio.ImageIO;
import java.awt.image.BufferedImage;
import java.io.File;
import java.util.*;

/**
 * Extracts individual sprites from the supplied source sheets.
 *
 * <p>The sheets arrive as flat JPEG/PNG with a chroma background (green, purple or black) and
 * several sprites laid out on one image. This keys the background to alpha, finds each connected
 * island of remaining pixels, and writes it out cropped to its own bounds.
 *
 * <p>Done as a tool rather than by hand because the alternative is eyeballing crop rectangles for
 * dozens of sprites, which is slow and gets redone every time a sheet changes.
 *
 * <p>Run: {@code java tools/AssetExtract.java <sheet-dir> <out-dir> [minPixels]}
 */
public final class AssetExtract {

    public static void main(String[] args) throws Exception {
        System.setProperty("java.awt.headless", "true");
        File srcDir = new File(args[0]);
        File outDir = new File(args[1]);
        int minPixels = args.length > 2 ? Integer.parseInt(args[2]) : 2500;
        outDir.mkdirs();

        File[] files = srcDir.listFiles();
        Arrays.sort(files, Comparator.comparing(File::getName));
        int written = 0;

        for (File f : files) {
            String n = f.getName().toLowerCase();
            if (!(n.endsWith(".jpeg") || n.endsWith(".jpg") || n.endsWith(".png")
                    || n.endsWith(".webp"))) continue;
            BufferedImage src;
            try {
                src = ImageIO.read(f);
            } catch (Exception e) {
                continue;
            }
            if (src == null) continue;

            boolean[] mask = null;
            List<int[]> boxes = null;
            int[] bg = null;
            for (int rank = 0; rank < 2; rank++) {
                int[] candidate = detectBackground(src, rank);
                if (candidate == null) continue;
                boolean[] m = buildMask(src, candidate);
                List<int[]> b = components(m, src.getWidth(), src.getHeight(), minPixels);
                // A key colour that leaves one sprite covering almost the whole sheet did not
                // actually key anything; prefer the candidate that separates more pieces.
                boolean degenerate = b.size() == 1
                        && (b.get(0)[2] - b.get(0)[0]) > src.getWidth() * 0.92f
                        && (b.get(0)[3] - b.get(0)[1]) > src.getHeight() * 0.92f;
                if (!degenerate && (boxes == null || b.size() > boxes.size())) {
                    mask = m;
                    boxes = b;
                    bg = candidate;
                }
            }
            if (bg == null || boxes == null) continue;
            String base = f.getName().replaceAll("[^A-Za-z0-9]+", "_").replaceAll("_+", "_");

            int index = 0;
            for (int[] b : boxes) {
                BufferedImage cut = cut(src, mask, b);
                if (cut == null) continue;
                ImageIO.write(cut, "png", new File(outDir, base + "_" + (index++) + ".png"));
                written++;
            }
            if (index > 0) {
                System.out.println(f.getName() + " -> " + index + " sprite(s)  bg=rgb("
                        + bg[0] + "," + bg[1] + "," + bg[2] + ")");
            }
        }
        System.out.println("extracted " + written + " sprites into " + outDir);
    }

    /**
     * Returns the background colour if a clear majority of the border is one colour, else null.
     *
     * <p>Sampling only the four corners was not enough: several sheets carry black letterbox bars
     * above and below the artwork, so the corners agreed on black while the actual key colour was
     * the green in between, and the whole sheet came out as a single sprite. Taking the mode of
     * the whole border ring picks the real key colour in that case, and requiring a majority still
     * rejects painted backdrops that have no key colour at all.
     */
    private static int[] detectBackground(BufferedImage img, int rank) {
        int w = img.getWidth(), h = img.getHeight();
        Map<Integer, Integer> counts = new HashMap<Integer, Integer>();
        int samples = 0;
        for (int x = 0; x < w; x++) {
            for (int y : new int[]{1, h - 2}) {
                int q = quantise(img.getRGB(x, y));
                counts.put(q, counts.getOrDefault(q, 0) + 1);
                samples++;
            }
        }
        for (int y = 0; y < h; y++) {
            for (int x : new int[]{1, w - 2}) {
                int q = quantise(img.getRGB(x, y));
                counts.put(q, counts.getOrDefault(q, 0) + 1);
                samples++;
            }
        }
        List<Map.Entry<Integer, Integer>> ranked =
                new ArrayList<Map.Entry<Integer, Integer>>(counts.entrySet());
        ranked.sort(new Comparator<Map.Entry<Integer, Integer>>() {
            public int compare(Map.Entry<Integer, Integer> a, Map.Entry<Integer, Integer> b) {
                return Integer.compare(b.getValue(), a.getValue());
            }
        });
        if (ranked.isEmpty() || ranked.get(0).getValue() < samples * 0.20) return null;

        int pick = ranked.get(0).getKey();
        // Letterboxed sheets have more black border than key colour, so the runner-up is tried
        // too and whichever colour yields more separable sprites wins. Keying the bars instead of
        // the chroma turned one such sheet into a single sprite covering the whole image.
        if (rank == 1) {
            if (ranked.size() < 2 || ranked.get(1).getValue() < samples * 0.12) return null;
            pick = ranked.get(1).getKey();
        }
        int[] first = {((pick >> 16) & 0xFF) * 8, ((pick >> 8) & 0xFF) * 8, (pick & 0xFF) * 8};
        boolean green = first[1] > 120 && first[1] > first[0] + 50 && first[1] > first[2] + 50;
        boolean purple = first[0] > 90 && first[2] > 140 && first[1] + 40 < first[2];
        boolean black = first[0] < 40 && first[1] < 40 && first[2] < 40;
        boolean white = first[0] > 235 && first[1] > 235 && first[2] > 235;
        return (green || purple || black || white) ? first : null;
    }

    /** Buckets a colour to 5 bits per channel so JPEG noise does not split the mode. */
    private static int quantise(int rgb) {
        return (((rgb >> 19) & 0x1F) << 16) | (((rgb >> 11) & 0x1F) << 8) | ((rgb >> 3) & 0x1F);
    }

    private static boolean[] buildMask(BufferedImage img, int[] bg) {
        int w = img.getWidth(), h = img.getHeight();
        boolean[] mask = new boolean[w * h];
        // JPEG ringing smears the key colour into the sprite edge, so the threshold is generous.
        int threshold = 90;
        for (int y = 0; y < h; y++) {
            for (int x = 0; x < w; x++) {
                mask[y * w + x] = dist(rgb(img, x, y), bg) > threshold;
            }
        }
        // Erode only chroma keys. On a green screen the fringe is a real halo worth removing, but
        // on a white or black sheet the content is usually line art a pixel or two wide, and
        // eroding it deletes the artwork outright - which is exactly what happened to the face
        // and expression sheets, where every slice came out as unconnected fragments.
        boolean chroma = isChroma(bg);
        return chroma ? erode(mask, w, h) : mask;
    }

    /** True for a saturated key colour, as opposed to a plain white or black sheet background. */
    private static boolean isChroma(int[] c) {
        int max = Math.max(c[0], Math.max(c[1], c[2]));
        int min = Math.min(c[0], Math.min(c[1], c[2]));
        return max - min > 60;
    }

    /** Removes single-pixel fringe left by JPEG compression around the key colour. */
    private static boolean[] erode(boolean[] mask, int w, int h) {
        boolean[] out = new boolean[w * h];
        for (int y = 1; y < h - 1; y++) {
            for (int x = 1; x < w - 1; x++) {
                int i = y * w + x;
                out[i] = mask[i] && mask[i - 1] && mask[i + 1] && mask[i - w] && mask[i + w];
            }
        }
        return out;
    }

    /** Flood-fills the mask and returns bounding boxes of islands above the size threshold. */
    private static List<int[]> components(boolean[] mask, int w, int h, int minPixels) {
        boolean[] seen = new boolean[w * h];
        List<int[]> boxes = new ArrayList<int[]>();
        int[] stack = new int[w * h];

        for (int start = 0; start < mask.length; start++) {
            if (!mask[start] || seen[start]) continue;
            int top = 0, count = 0;
            stack[top++] = start;
            seen[start] = true;
            int minX = w, minY = h, maxX = 0, maxY = 0;

            while (top > 0) {
                int i = stack[--top];
                int x = i % w, y = i / w;
                count++;
                if (x < minX) minX = x;
                if (x > maxX) maxX = x;
                if (y < minY) minY = y;
                if (y > maxY) maxY = y;
                // 8-connected: fins and whiskers often touch the body only diagonally.
                for (int dy = -1; dy <= 1; dy++) {
                    for (int dx = -1; dx <= 1; dx++) {
                        int nx = x + dx, ny = y + dy;
                        if (nx < 0 || ny < 0 || nx >= w || ny >= h) continue;
                        int j = ny * w + nx;
                        if (mask[j] && !seen[j]) {
                            seen[j] = true;
                            stack[top++] = j;
                        }
                    }
                }
            }
            if (count >= minPixels) boxes.add(new int[]{minX, minY, maxX, maxY});
        }
        // Largest first, so the interesting sprites land at index 0.
        boxes.sort(new Comparator<int[]>() {
            public int compare(int[] a, int[] b) {
                return Integer.compare((b[2] - b[0]) * (b[3] - b[1]), (a[2] - a[0]) * (a[3] - a[1]));
            }
        });
        return boxes;
    }

    private static BufferedImage cut(BufferedImage src, boolean[] mask, int[] box) {
        int w = box[2] - box[0] + 1, h = box[3] - box[1] + 1;
        if (w < 24 || h < 24) return null;
        BufferedImage out = new BufferedImage(w, h, BufferedImage.TYPE_INT_ARGB);
        int sw = src.getWidth();
        for (int y = 0; y < h; y++) {
            for (int x = 0; x < w; x++) {
                int sx = box[0] + x, sy = box[1] + y;
                if (mask[sy * sw + sx]) out.setRGB(x, y, src.getRGB(sx, sy) | 0xFF000000);
            }
        }
        return out;
    }

    private static int[] rgb(BufferedImage img, int x, int y) {
        int p = img.getRGB(x, y);
        return new int[]{(p >> 16) & 0xFF, (p >> 8) & 0xFF, p & 0xFF};
    }

    private static int dist(int[] a, int[] b) {
        return Math.abs(a[0] - b[0]) + Math.abs(a[1] - b[1]) + Math.abs(a[2] - b[2]);
    }
}
