package com.vancan.autofishing.sim;

/**
 * Deterministic PRNG (SplitMix64) used by every part of the simulation.
 *
 * <p>{@link java.util.Random} is deliberately avoided: its behaviour is only guaranteed for
 * {@code nextInt}/{@code nextDouble} on the JVM, and the GWT emulation is not bit-identical.
 * The fishing core has to replay a session from a seed on the client, on the server and in the
 * balance harness and reach the exact same result, so the generator ships with the game.
 */
public final class Rng {

    private long state;

    public Rng(long seed) {
        this.state = seed;
    }

    /** Derives an independent, reproducible stream from a parent seed and a label. */
    public static Rng derive(long seed, String label) {
        long h = seed;
        for (int i = 0; i < label.length(); i++) {
            h ^= label.charAt(i);
            h *= 0x100000001b3L;
        }
        return new Rng(h);
    }

    public long nextLong() {
        state += 0x9E3779B97F4A7C15L;
        long z = state;
        z = (z ^ (z >>> 30)) * 0xBF58476D1CE4E5B9L;
        z = (z ^ (z >>> 27)) * 0x94D049BB133111EBL;
        return z ^ (z >>> 31);
    }

    /** Uniform in [0,1). Uses 53 bits so the result is exact across JVM and GWT doubles. */
    public double nextDouble() {
        return (nextLong() >>> 11) * 0x1.0p-53;
    }

    public float nextFloat() {
        return (float) nextDouble();
    }

    /** Uniform in [0,bound). */
    public int nextInt(int bound) {
        if (bound <= 0) throw new IllegalArgumentException("bound must be > 0, was " + bound);
        return (int) (nextDouble() * bound);
    }

    public double range(double min, double max) {
        return min + nextDouble() * (max - min);
    }

    public boolean chance(double p) {
        return nextDouble() < p;
    }

    /**
     * Triangular-ish roll biased towards the low end, used for fish weight rolls so that
     * "a big one" stays rare inside a species' own weight band.
     */
    public double weighted(double min, double max, double bias) {
        double u = nextDouble();
        double curved = Math.pow(u, bias);
        return min + curved * (max - min);
    }

    public long getState() {
        return state;
    }

    public void setState(long state) {
        this.state = state;
    }
}
