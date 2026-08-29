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
| Directory | given, not guessed — see below | yes, as a patch |

## A game that is a folder

A PC game is installed, not shipped: it sits on disk as a directory of forty thousand files. That
does not fit "read the bytes of one archive", so importing a directory is two steps, and keeping
them apart is the design.

**The scan opens nothing.** It walks the tree and records paths and sizes. That is what lets the
tool say *"41 812 files, 23 read"* before a single file is opened — and the order of those two
numbers matters, because "23 files" on its own reads like something went wrong.

**Ingestion then reads only what is worth reading**: files whose extension is one this build
actually does something with (`properties`, `strings`, `xml`, `json`, `po`, `ini`, `txt`, `rpy`,
`locres`, `csv` and a few more), under 8 MiB each and 64 MiB in total. What it produces is an
`Archive` — the same type an APK or a JAR becomes — which is why detection, extraction, the build,
the rules and validation all run on a directory without a line of change.

Files passed over come in two kinds and are treated differently. A texture is not mentioned: four
hundred lines saying "this PNG is not text" would bury the one line that matters. A **text file
skipped for its size is named, with its size and the reason**, because a 300 MB JSON quietly
dropped is exactly what a translator finds out about far too late.

What was read is **copied** into `original/tree/`, not merely hashed — so the project still holds
the bytes it started from after Steam has updated over the game, or the drive was unplugged. The
tree is pinned by the hash of a manifest of those copies, which keeps `verify_original` a single
comparison; and it re-hashes the copies on disk rather than reading the recorded hashes back,
because comparing a record with itself is a check that checks nothing.

The files that were **not** read were not hashed either. `original/tree.json` says so in as many
words rather than leaving a whole-game guarantee to be assumed: hashing forty gigabytes on every
open is not something anyone would wait for.

The engine is guessed from the file names the scan already collected — a Unity `*_Data` directory,
`project.godot`, `Engine/Binaries/`, `steam_api64` — and reported as **evidence**, beside what was
concluded, so a wrong answer can be argued with.

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
| `renpy` | `translate <language> <label>:` blocks | Ren'Py, so a great many indie and visual-novel games |
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

## Ren'Py: two files with one extension, and opposite answers

Ren'Py is translated through files **its own tooling generates**, under `game/tl/<language>/`. Those
carry the original beside an empty slot, the way a `.po` file does, in two shapes:

```renpy
translate vietnamese start_a1b2c3:

    # e "Cast your line."      ← the original, in a comment
    e ""                       ← the slot

translate vietnamese strings:

    old "Start Game"
    new ""
```

Both are read and both are written. Addresses follow one rule, `<block label>::<what distinguishes
it>`: a dialogue line gives `start_a1b2c3::0`, `start_a1b2c3::1`, because a block holds several
lines and a label alone would let the second translation overwrite the first; the strings block
gives `strings::Start Game`, because there the original is the key Ren'Py itself matches on. The
ordinal is a position, so inserting a line into a block shifts the ones after it — but a node id
hashes the source text as well as the address, so a shifted line gets a **new** id and its old
translation is dropped rather than misapplied. Lost work, never wrong text.

**The game's own `.rpy` script is read-only, and that is the point of the rule.** A script decodes
as text and is full of dialogue, so with no rule at all it would fall to the `lines` reader — which
offers every non-blank line, `label start:` and `$ points += 1` among them, and writes back by
replacing whole lines. One approved line and the game stops parsing. So the script is refused by
name, with a reason that says where translations do go. Both files end in `.rpy`; only the contents
tell them apart, which is why the `translate` header is what is confirmed and the extension is only
the hint.

A third thing shares the name and is still not read at all: `.rpa`, Ren'Py's **archive**, which
needs its own container reader.

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

## Đọc được, ghi được, và chỗ ở giữa

Ba trạng thái, không phải hai. `writeback::plan` quyết định cả ba, ở **một chỗ**, và extraction,
build lẫn khảo sát gói đều hỏi nó:

| Trạng thái | Nghĩa là |
| --- | --- |
| `Text` | Chữ, sửa tại chỗ qua `resource::write` |
| `Binary` | Định dạng nhị phân có reader **và** writer riêng (hiện chỉ `.locres`) |
| `ReadOnly` | Bản build này không ghi được — lý do đến thẳng người dùng |

**Mặc định là từ chối.** Byte không đọc ra chữ và không khớp writer nào thì là `ReadOnly`, và một
resource `ReadOnly` **không bao giờ bị đụng vào**.

Đó không phải cẩn thận suông. Trước khi có module này, build decode mọi resource bị vá bằng
`from_utf8_lossy` rồi ghi đè. Chưa có node nào sinh ra từ file nhị phân nên chưa hỏng gì — nhưng
reader DEX đầu tiên sẽ biến mọi byte không hợp lệ của `classes.dex` thành U+FFFD, ghi đè, và **báo
build thành công**. `tools/verify-packages.sh` giờ khẳng định `classes.dex`, `resources.arsc` và
`AndroidManifest.xml` ra khỏi bản build **y hệt byte đầu vào**. Fixture tự sinh chứng minh mệnh đề
phủ định một cách trọn vẹn: "những byte này không bị đụng vào" không cần game thật.

Một bản dịch đã duyệt nhắm vào file không ghi được sẽ **không im lặng biến mất**. Build ghi lại một
`Refusal` kèm số lượng, và kiểm tra `text.unwritable` báo một dòng cho cả file: *"412 bản dịch đã
duyệt trong classes.dex sẽ không xuất hiện: bản build này đọc được bảng chuỗi nhưng chưa ghi lại
được."* Một dòng, không phải 412 — 412 dòng giống nhau là báo cáo không ai đọc.

Việc gom về một chỗ cũng xoá hai bất đồng đang có. `extract` từng nhận `.locres` **theo đuôi file và
nuốt lỗi parse** trong khi khảo sát nhận **theo magic byte và báo lỗi**, nên `analyze` có thể nói
một file có ba chuỗi đọc được còn `extract` không ra node nào. Và khảo sát từng liệt kê
`META-INF/MANIFEST.MF` là "chữ đọc được" trong khi extraction cố tình không bao giờ đưa nó ra — hứa
với người dịch những chuỗi họ sẽ không bao giờ thấy.

## What is named but not read

Listed rather than passed over, each with what it is: `classes.dex`, `resources.arsc`, Unity
asset bundles, Godot `.pck`, Ren'Py `.rpa`, and a Ren'Py `.rpy` script. A translator who cannot see
that a game keeps half its dialogue in one of those will conclude the game is half translated when
it is not.

The `.rpy` script is the odd one on that list, and worth saying so: it is not refused because this
build cannot read it — it reads perfectly well — but because rewriting it is not how Ren'Py is
localized. Everything else there is a format waiting for a reader.

The DEX string pool is a documented format and readable in principle. It is not read yet, and the
list says so rather than staying quiet - which matters most for Android, where a game that keeps
its text in code rather than in `strings.xml` will look, from the extracted text alone, like a
game with almost nothing to translate.

## A guess is not a finding

Everything above is mechanical: a magic number, a table this build knows how to walk, a name on a
list somebody wrote down. Turning on the analysis engine (see `LANGUAGES.md`) adds a second, softer
kind of answer - Claude reading a listing of file names and saying which ones look like they hold
text a player would read.

The two are kept apart everywhere they appear:

- Suggestions have their **own section** in `analyze` and in the Overview tab, with the reason and
  the model that gave it. They never join the readable list.
- Suggestions are stored in `content/suggestions.json`, not in the graph.
- `writeback::plan` **never consults them**. What can be written back stays a decision made from
  bytes, because a file written back on a guess is a destroyed file, and the build would report
  success.

The scan itself sends only names, sizes and what the mechanical check already concluded; asking
about one file sends the first 2 KiB of that one file. Neither ever sends a package's contents
wholesale.

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
