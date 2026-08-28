package com.vancan.autofishing.content;

/**
 * Where the content loader gets its JSON from.
 *
 * <p>Deliberately not {@code FileHandle}: the loader then works unchanged under plain java.io in
 * unit tests and the balance harness, under libGDX assets on device, and (later) over an HTTP
 * Remote Config fetch, without any of them dragging in the others.
 */
public interface TextSource {
    /** @param path path relative to the data root, e.g. {@code "species.json"} */
    String read(String path);
}
