# What this tool can open

The project began with J2ME JARs, and the rest of the world does not ship those.

An Android game is an APK, an iOS one an IPA, a PC game a folder or a zip. All of them are ZIP
archives underneath, so the archive layer already read them; what was missing was knowing which
one is in front of you, reading the text formats each one actually uses, and being honest about
what can then be done with it.

## Recognising it

From what is inside, never from the extension:

| Kind | Recognised by | Can be rebuilt here |
| --- | --- | --- |
| J2ME MIDlet | a `MIDlet-1` attribute in the manifest | yes |
| Java archive | class files, no MIDlet attribute | yes |
| Android package | `AndroidManifest.xml` and a `classes.dex` | **no** |
| iOS application | a `Payload/*.app/` directory | **no** |
| Zip of files | none of the above | yes |

## The two that cannot be rebuilt

An Android package is signed, and rewriting it breaks the signature: the device refuses to install
the result until somebody re-signs it with their own key. An iOS application is signed *and*
provisioned for particular devices, and neither can be redone without an Apple developer identity.

Neither is "unsupported". The text is read, translated, checked and written back, and the archive
this tool produces is exactly what `apksigner` takes as input. What it will not do is hand
somebody a file and let them find out at install time - so the build says so, every time, in the
build record:

```
warn package.signature  this is an android package: an Android package is signed, and rewriting it
                        breaks the signature; the device will refuse to install the result until
                        somebody re-signs it with their own key
```

The output also keeps the extension it came in as. An APK saved as `.jar` is still an APK, and
nothing that opens it knows that - starting with the person looking for it in a file manager.

## The text formats

A key, a value, and a lot of surrounding text that must survive untouched. That last part is the
difficulty: a file rewritten from a parsed model loses its comments, its ordering and its spacing,
and the game's own tooling then reports a diff nobody made. So every writer edits in place.

| Format | Looks like | Ships in |
| --- | --- | --- |
| `properties` | `key=value` | J2ME, Java |
| `android-strings` | `<string name="key">value</string>` | Android |
| `apple-strings` | `"key" = "value";` | iOS, macOS |
| `gettext` | `msgid` / `msgstr` | a great many PC games |
| `json` | any string in the document | Unity, RPG Maker, most engines |
| `ini` | `[section]` over `key=value` | older PC games |
| `unreal-locres` | a compiled binary string table | Unreal Engine, so a great many Steam games |
| `lines` | every non-empty line | anything else that decodes as text |

The format is decided by the name *and* the contents. Neither is enough on its own: plenty of
games ship JSON in a `.txt`, and a gettext catalogue and a properties file both look like lines
with an equals sign in them.

JSON is the one format that does not preserve its own layout, because a JSON document has no line
structure worth preserving - it is a serialisation, not something a person maintains. Strings in
it are addressed by path (`actors[1].name`) rather than by name, because the shape of a game's
JSON is the game's business and two objects using the same field name must not collide.

Where a format has keys they are used, because a key still finds its value after somebody edits
the file and a line number does not. INI keys are qualified by their section (`menu.title`), since
two sections may hold the same key and a reader that ignored sections would translate one and
silently overwrite the other.

## Unreal's string table

`.locres` is the one binary format read directly here, because it is self-contained and
documented: a header, a table of namespaces and keys, and an array of the strings themselves.

Two things it does deliberately.

**It refuses rather than guesses.** Anything whose magic or version is not exactly what the parser
knows is rejected with a reason. Version 1 keeps its strings inline instead of in a shared array,
and is declined rather than read on a hopeful interpretation - a binary format read slightly wrong
produces text that looks almost right and a file that crashes a game, and the second is discovered
long after the first.

**Everything it does not translate is carried through.** Namespaces, keys and the source-text
hashes are written back exactly as read. The hashes are Unreal's way of noticing a translation has
gone stale, and inventing new ones would tell the engine that every string had been re-checked
against a source nobody looked at.

Unreal stores each distinct text once and counts the references, so two entries can share a
string. Translating one gives it its own copy rather than changing both.

**The caveat, stated plainly.** This reader was written from the documented layout, and the tests
prove that it and the writer agree with each other and that everything untranslated survives.
There is no shipped Unreal game here to check against, so it has not been proved that a real
engine loads what this produces. `tools/verify-packages.sh` reads the built table back with an
independently written parser, which rules out one class of mistake and not that one. Anyone
putting a real game through it should check the result in the game before trusting it.

## What is named but not read

Listed rather than passed over, each with what it is: `classes.dex`, `resources.arsc`, Unity
asset bundles, Godot `.pck`, Ren'Py `.rpa`. A translator who cannot see that a
game keeps half its dialogue in one of those will conclude the game is half translated when it is
not.

The DEX string pool is a documented format and readable in principle. It is not read yet, and the
list says so rather than staying quiet - which matters most for Android, where a game that keeps
its text in code rather than in `strings.xml` will look, from the extracted text alone, like a
game with almost nothing to translate.

## Rules that belong to one format

Validation asks JAR questions only of JARs. A MIDlet entry point and a glyph sheet are rules about
one packaging format, and reporting them as missing from an Android package would be reporting
that it is not a J2ME game - which nobody needed telling, and which is how a person learns to
ignore the report.

The same goes for what is never offered as game text. `META-INF/MANIFEST.MF` was excluded early,
after `MIDlet-1: Sample Game,/icon.png,SampleGame` reached a translator and translating it renamed
a game's entry point. `AndroidManifest.xml` and `Info.plist` are the same file in a different
costume - the package name, the permissions, the class of every screen - and are excluded for the
same reason. `InfoPlist.strings`, which is the localizable one, is not.
