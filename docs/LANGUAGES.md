# Languages, dictionaries and register

What the tool can honestly do about turning one language into another, and where the line is.

## The line, stated first

**A dictionary cannot translate a sentence.** Word order, agreement, classifiers and idiom are not
in a dictionary, and stitching readings together in source order produces something that looks
like a translation and is not one.

What a dictionary *can* do is resolve terminology, and terminology is most of what makes a game
translation read like a game rather than like a manual:

| Source | A general dictionary says | A game dictionary says |
| --- | --- | --- |
| `装备` | thiết bị (hardware) | **trang bị** |
| `Guild` | hiệp hội (a trade body) | **bang hội** |
| `法力` | pháp lực | **nội lực** in wuxia |
| `EXP` | — | **kinh nghiệm** |

For short interface strings - the majority of a J2ME game's text - that is a complete answer. For
a sentence it is a starting point, and the tool says so rather than presenting a gloss as finished
work.

Three rules enforce that, and they are tested:

1. **No gloss is ever auto-approved**, however complete. `Proposal::is_approvable` returns false
   unconditionally. What may be approved without a person is a decision a person already made: an
   exact translation-memory hit, or a locked glossary term.
2. **A mostly unresolved string is not glossed at all.** "Dragon Quest Online" is a title; the
   dictionary knows only "Quest", and substituting it gives "Dragon nhiệm vụ Online" - which looks
   like an attempt, invites a tired reviewer to accept it, and is worse than proposing nothing.
   Below half covered, the engine says nothing.
3. **Unresolved text is named, not hidden.** A gloss reports which stretches nothing covered, so
   the gap is visible.

## Register: why this matters more in Vietnamese

Vietnamese has no neutral second person. "Are you sure?" has one dictionary reading and several
right answers, and choosing between them is not a vocabulary question:

| Register | The same line |
| --- | --- |
| `natural-dialogue` (kiếm hiệp / tiên hiệp) | Ngươi chắc chứ? |
| `modern` | Bạn có chắc không? |
| `formal` (shops, payments) | Quý khách có chắc chắn không? |
| `terse-ui` (buttons and labels) | *no pronoun at all* |

A translator who ignores this produces text that reads as a machine's. Worse, a game translated by
several people drifts between registers, which readers notice even when they cannot name it.

So the register is a project setting, applied to every line, and wording that breaks it is
reported: `bạn` in a wuxia game, `ngươi` in a modern one. The check is word-boundary aware, because
Vietnamese words are short and sit inside one another - `ta` is inside `hoàn tất` and `tay`, and
neither is the pronoun.

The register is **reported, never rewritten**. Substituting `ngươi` for `bạn` inside a finished
sentence leaves the rest of the sentence built around the wrong reading.

The same problem exists in Japanese and Korean, and more weakly in the T/V distinction of Russian,
German and French. This build models it for Vietnamese only; other languages get a profile that
checks nothing, and says so rather than pretending.

## What ships

633 entries across eight directions:

| Direction | Entries |
| --- | --- |
| zh → vi-VN | 159 |
| en → vi-VN | 144 |
| ja → vi-VN | 60 |
| ko → vi-VN | 60 |
| ru → vi-VN | 54 |
| en → zh-Hans | 52 |
| en → th | 52 |
| en → id | 52 |

Embedded in the binary, so the tool works without a data directory beside it. A project's own packs
go in its `dictionary/` folder and are loaded on top.

Four English terms carry more than one reading: the established one, and a shorter label for a
narrow button - "Start Game" as *bắt đầu trò chơi*, *bắt đầu* or *chơi*. The full rendering has the
higher priority and stays the one the tool proposes; the short ones exist for `shorten`, which
offers them only when the layout check has measured the label as too wide (see `docs/FONTS.md`).

Priority decides between readings of one term, which it could not do until recently: a `ui` entry
in a `ui` context already scores the maximum, the score was clamped after priority was added, and
so readings tied and the listing order won. A curator raising a priority saw nothing change, which
is worse than having no priority at all.

Entries carry a **domain** - `ui`, `combat`, `item`, `skill`, `quest`, `social`, `stat`, `system`,
`story` - matched against the content node's context, so a term can read differently in a menu and
in combat text.

## The source language matters more than it looks

Every dictionary is keyed by direction, so a wrong source language silently disables all of them
and the tool quietly stops proposing anything. It is therefore **detected** on import, by counting
scripts across the extracted strings, and recorded as a guess - the interface says so, in a banner
that does not go away until someone confirms it.

Japanese is checked before Chinese: Japanese text is mostly Han characters with some kana, so any
kana at all outweighs a large Han count.

## Several target languages at once

One project, one body of extracted text, several targets. Each target owns its translations,
glossary, memory, register and builds; they are separate bodies of work, reviewed separately.

```
translations/vi-vn.json      builds/vi-vn/0001/     output/game-vi-vn.jar
translations/th.json         builds/th/0001/        output/game-th.jar
memory/en-vi-vn.json         glossary/vi-vn.json
```

Removing a language from the profile leaves its files alone. Deleting a body of reviewed work
because a checkbox was cleared is not a thing a tool should do; re-adding the language picks it
straight back up.

## An external engine

`translate::Provider` is the seam; `DictionaryProvider` is the offline implementation and the
default. `provider::HttpProvider` talks to an engine over the network, and it is **off unless
switched on**. Nothing reaches the network while it is off - there is a test for that, because it
is the guarantee the offline default rests on.

No service is built in. Which one, at what price, under whose terms, is the user's decision. What
is built in is the shape of the request for five API families, so configuring one is a URL and a
key rather than a plugin:

| Family | Notes |
| --- | --- |
| `openai-compatible` | OpenAI and everything that copied its `/chat/completions`, including local runtimes. The only family that can be **told** the register and terminology in words. |
| `deepl` | `/v2/translate`, with sentence splitting off - game strings are fragments, and splitting them invents sentences. |
| `google-v2` | Takes its key in the query string, not a header. |
| `libretranslate` | Including a self-hosted one, where nothing leaves a network the user controls. |
| `anthropic` | `/v1/messages`. The other family that can be **told** the register and terminology; the briefing goes in the `system` block with `cache_control`, because it is the same for every string in a run. Defaults to `claude-opus-5`. A decline arrives as a successful response with `stop_reason: "refusal"`, so that is checked before the content is read. |

### What makes it a game translation rather than a correct one

An engine that knows nothing about this game renders `装备` as "thiết bị", `Guild` as "hiệp hội",
and addresses a wuxia player as "bạn". Every one of those is fluent, grammatical and wrong.

So the project's terminology and register go **into** the request:

```
You are translating text from a video game, from English into Vietnamese.
Reply with the translation only: no quotes, no explanation, no alternatives.
Keep these placeholders exactly as they are, in the same number: %d.
This string appears in the game's ui text. Keep it about as short as the original.
Register: Kiếm hiệp / tiên hiệp: ta - ngươi, archaic and distant.
This is interface text with no speaker: use no personal pronouns at all.
Never write "bạn"; write "ngươi".
Use these renderings exactly; they are settled for this game:
  Guild = bang hội
```

And the same things are checked **coming back**, because no engine reliably follows a briefing:

- A reply that **lost a placeholder is refused outright**, not flagged. Applying it would break the
  game at runtime, and no amount of fluency makes that usable.
- A reply that ignored a **settled glossary term** is offered with the disagreement named, and its
  confidence drops. Flagged rather than rewritten: the sentence may have been built around the
  wrong reading.
- A reply that breaks the **register** is flagged the same way. This is the check that earns its
  keep: a modern pronoun in a wuxia game passes every grammatical test there is.
- Quotes the engine added despite being asked not to are stripped, because a quoted string patched
  into a game shows the quotes.
- A service that answers 200 with an error object reports the service's own wording rather than
  "no translation".

**No machine translation is ever auto-approved.** Fluent and wrong is the failure mode, and nothing
in a reply distinguishes it from fluent and right. Confidence caps at 0.7 for a clean reply and
0.4 for one with a warning, and `is_approvable` is false regardless.

### Where the key lives

Not in `project.json`, and nowhere under the project directory. A project is a folder people
commit, zip up and send to a translator; a key in it leaks the first time anyone does any of that.
It goes in the application's own configuration directory, in a file readable only by its owner
(`0600` on Unix; on other platforms the permission call is a no-op and this document says so
rather than implying protection that is not there).

This is not a secret store. It is a file with tight permissions, which is what a desktop
application can offer without a platform keychain.

Keys are filed **by endpoint URL**, not by family. So the `anthropic` engine and the analysis side
below share one key without either being told about the other: they name the same endpoint.

**The command line and the application read different directories.** The CLI uses
`$XDG_CONFIG_HOME/com.thanhtinz.tjlocalizer` (falling back to `~/.config`); the application uses
Tauri's own `app_config_dir()`, which is that path on Linux but not on macOS or Windows. A key
stored in the application is therefore not found by the CLI on macOS or Windows, and the symptom is
"no key stored" on a machine where one plainly is. Store it in whichever of the two you use, or
both. Unifying them would move a key file that already exists on somebody's machine, so it is
written down here rather than changed quietly.

### Seeing what would go

`tjlocalizer engine <project> --dry-run "some text"` prints the exact request without sending it,
and the interface has the same thing behind **Xem thử request**. A user about to send their game's
text to a third party should be able to see precisely what would go.

## Asking Claude about the files, rather than about the words

A separate seam, `claude::Analyst`, for the three questions the mechanical checks cannot answer:
which files in a package look like they hold text, what an unknown file is, and what looks wrong
with translations somebody has already approved. It is **off by default, and off means nothing is
sent** - the same guarantee the engine above makes, kept by the same kind of test, which counts how
many times the transport was reached.

Three rules hold across all of it:

**What goes out is bounded, and it is visible before it goes.** A scan sends *file names, sizes and
what the mechanical check already made of each*. It does not send file contents. A sample - the
first 2 KiB - of exactly one file goes only when somebody asks about that one file. `tjlocalizer
analyze <project> --with-claude` prints the count first; `tjlocalizer inspect <project> <entry>
--dry-run` and `tjlocalizer review <project> --dry-run` print the whole request and send nothing.
The application shows the file list itself under **Xem trước sẽ gửi gì**.

**The token count is measured, not estimated.** It comes from `POST /v1/messages/count_tokens`.
Characters divided by four is a guess, and calling a guess an estimate is the kind of half-honesty
this project does not do. When the count cannot be got, the interface says so instead of showing a
number.

**Nothing that comes back becomes a fact.** Suggestions are shown in their own section, labelled as
guesses, with the model that made them named; they are stored in `content/suggestions.json`, apart
from the graph. They never enter `package::survey`'s readable list and they are never consulted by
`writeback::plan` - what a build writes back stays a mechanical decision, because writing a file
back on a guess destroys it. Review results are **notes on rows, never edits**, the same rule that
makes `Proposal::is_approvable()` false unconditionally.

`review` is the one path that sends the game's own text and its translation, so it sits behind its
own deliberate action and prints how many lines it would send before it sends them.

There is **no `tools/verify-*.sh` for this**, unlike every other part of the project: it would need
a real key and a real network, and continuous integration has neither. What is proved offline is
proved thoroughly - `crates/tjlocalizer-core/tests/claude_tests.rs` asserts that nothing is sent
while it is off, and that a fixture's own contents do not appear in a scan's request body - but no
test here shows a real answer from a real service.

## What is not built

- Register modelling for any language but Vietnamese.
- Grammar beyond spacing, length, script and placeholder checks.
- A speaker and relationship model that assigns a stance per line automatically (§15). The types
  exist and default to neutral; nothing populates them yet.
- Segmentation for Thai and Japanese, which run words together. Dictionary matching works on those
  scripts because it does not need word boundaries, but a length or word-count rule would.
