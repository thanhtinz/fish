# Building

Requires **JDK 17+** (the shared module targets Java 11 bytecode for GWT and RoboVM
compatibility). Everything below uses the Gradle wrapper.

## Verification status

Honest account of what was actually run:

| Target | Status |
|---|---|
| `:core` tests (29) | ✅ Run, all pass |
| Desktop (LWJGL3) | ✅ Built and run; all five screens captured under Xvfb |
| HTML5 (GWT) | ✅ Compiled, served, and driven in headless Chromium with no console errors |
| iOS (RoboVM) | ⚠️ Java sources compile and dependencies resolve. Linking an `.ipa` needs macOS + Xcode, which was not available. |
| Android | ⚠️ Module and manifest are complete, but nothing was built: the Android SDK could not be installed here because `dl.google.com` is unreachable from this environment. |

---

## Desktop (development harness)

```bash
./gradlew :lwjgl3:run
```

Opens a 495x880 window matching the 9:16 design ratio. This is a development target, not a
shipping one.

### Screenshot harness

Renders each screen and writes PNGs, for reviewing layout without opening the app by hand:

```bash
./gradlew :lwjgl3:run --args="--screenshot /tmp/shots"
# headless (CI):
xvfb-run -a -s "-screen 0 1080x1920x24" ./gradlew :lwjgl3:run --args="--screenshot /tmp/shots"
```

---

## HTML5

```bash
./gradlew :html:dist
cd html/build/dist && python3 -m http.server 8080
```

Output is a fully static directory (~660 KB) — no server-side component. Deploy it to any
static host.

For an iterative dev loop with recompile-on-refresh:

```bash
./gradlew :html:gwtDev
```

**Note on the asset manifest.** GWT cannot list a directory over HTTP, so libGDX's preloader
reads `assets/assets.txt`. The GWT plugin generates it (with cache-busting hashes) from the
`gdx.assetpath` property in `GdxDefinition.gwt.xml`. Do not hand-write that file — a file
missing from it is simply absent at runtime, with no error.

---

## Android

Needs the Android SDK. Point Gradle at it with either `ANDROID_HOME` or a `local.properties`
containing `sdk.dir=/path/to/Android/sdk`. **The `:android` module is only included in the build
when one of those is present** (see `settings.gradle`), so a machine without the SDK can still
run the tests and build the web target.

```bash
export ANDROID_HOME=$HOME/Android/Sdk
./gradlew :android:assembleDebug        # -> android/build/outputs/apk/debug/
./gradlew :android:assembleRelease      # signed release; see below
./gradlew :android:bundleRelease        # .aab for Play
```

- Portrait is locked in `AndroidManifest.xml`; `minSdk` is 21, `targetSdk` 35.
- Release builds run R8. `proguard-rules.pro` keeps the libGDX classes that are reached only
  from native code — without them the release build crashes at start-up while debug works.
- Native `.so` files are unpacked into `android/libs/` by `copyAndroidNatives`, which is wired
  into the JNI merge task automatically.
- Signing is not committed. Configure it with a `signingConfigs` block reading from environment
  variables or a keystore properties file kept out of version control.

`google()` is declared **only** in `android/build.gradle`. Declaring it for every project made
unrelated dependency resolution fail on networks that answer 403 for that host, because Gradle
treats a 403 from a repository as an error rather than "not found, try the next one".

---

## iOS

Needs **macOS with Xcode**. The module is excluded from the build unless you opt in, since the
RoboVM plugin is useless elsewhere:

```bash
./gradlew -PwithIos :ios:launchIPhoneSimulator
./gradlew -PwithIos :ios:createIPA
```

- Portrait is locked in `Info.plist.xml`; minimum iOS 12.
- Signing reads `IOS_SIGN_IDENTITY` and `IOS_PROVISIONING_PROFILE` from the environment.
- `robovm.xml` force-links the classes RoboVM's dead-code stripper cannot see are used, because
  libGDX reaches them from native code.

---

## Content and asset tooling

Generated output is committed; re-run these only when changing the inputs.

```bash
java tools/FontGen.java        # assets/fonts/game.{fnt,png} - the Vietnamese glyph atlas
java tools/SpriteGen.java      # assets/sprites/atlas.{png,json} - fish, angler, boat, portraits
java tools/IconGen.java        # Android launcher icons + web logo
python3 tools/gen_species.py   # assets/data/species.json from the tier curves
```

After changing content, always run `./gradlew :core:test`: it validates the tables and checks
the balance curves and font coverage.
