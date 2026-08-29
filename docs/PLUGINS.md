# Plugins: adapters written as data

The core knows formats, never games. That rule is in `lib.rs` and it is what lets this tool open a
game nobody here has seen: it reads archives, class files, character sets and text resources, and
nothing in it could name a game even if somebody wanted it to.

It is also the limit. A game that keeps its text in `data/lang/en.txt` in a shape no detector
recognises is invisible to a tool that will not be told about it, and there is no general answer to
be found - the knowledge is genuinely specific to that game or that engine, and somebody has to
write it down.

Rules (§19) write down one half of that: what to change in one game. Plugins write down the other:
what one game or engine *is*.

## What a plugin is

A JSON file in the project's `plugins/` directory. Every file there is loaded, in alphabetical
order.

```json
{
  "id": "some-engine",
  "description": "this engine keeps its interface text under data/lang",
  "author": "whoever wrote it",

  "capabilities": [
    {
      "id": "some_engine",
      "confidence": 0.9,
      "when": [
        { "kind": "entryMatches", "pattern": "data/lang/*" },
        { "kind": "entryCount", "pattern": "*.assets", "atLeast": 4 }
      ]
    }
  ],

  "resources": [
    { "pattern": "data/lang/*.txt", "format": "properties", "note": "key=value, no sections" }
  ],

  "fonts": [
    { "pattern": "gfx/font*.png", "cellWidth": 12, "cellHeight": 12, "columns": 16 }
  ],

  "rules": [ { "id": "widen", "description": "...", "when": [], "then": [] } ],

  "dictionary": {
    "from": "en", "to": "vi", "name": "engine terms", "sourceNote": "the plugin",
    "entries": [{ "source": "Rune", "target": "Cổ Ngữ", "domain": "item", "note": "" }]
  }
}
```

Every section is optional. A plugin that only says "this file is a properties file" is a useful
plugin.

## It is data, and only data

No code is loaded and nothing is executed. This is a refusal, not an unfinished feature.

§29 treats a game archive as untrusted input, because it is: somebody downloaded it from a forum.
A plugin arrives by exactly the same route - somebody posts it beside the game. A plugin format
that could run code would make "open this game" mean "run this stranger's program", and every
guarantee this crate makes would be worth nothing.

So a plugin cannot read a format no reader here owns, cannot write bytecode, cannot reach the
network, and cannot touch a file outside the project. A game needing any of those needs a change
to the core, and a plugin file is the wrong place to hide it.

## What each section does

| Section | What it contributes | Where it lands |
| --- | --- | --- |
| `capabilities` | Capabilities reported for archives that match | The manifest (§6), each carrying `plugin <id>` as evidence |
| `resources` | Files read and written as a format this build already has | Extraction and the build, through `writeback::plan` |
| `fonts` | A guess at where the glyph sheet is and how it is laid out | Offered when picking a font (§16); the project still records the choice |
| `rules` | Rules offered to the project, switched off | The rule list (§19), under `<plugin id>:<rule id>` |
| `dictionary` | Terms this engine uses | The dictionary (§12), after the project's own packs |

The formats a `resources` entry may name are the ones with both a reader and a writer:
`properties`, `apple-strings`, `android-strings`, `gettext`, `ini`, `json`, `renpy`, `lines`.
Anything else is reported as a broken plugin rather than quietly ignored - a plugin contributing
nothing looks exactly like a plugin that had nothing to contribute, and the two need telling apart.

A resource claim is consulted *after* the binary readers and before the text detectors. A plugin
exists to name a file whose shape nothing here recognises; it has no business overruling a reader
that parsed the bytes and knows what they are.

## Patterns

`*` matches any run of characters, `?` matches one, and everything else is literal. Matched against
the whole entry name, so `data/lang/*.txt` matches `data/lang/en.txt` and not `other/lang/en.txt`.

Not regular expressions, on purpose: a plugin is written by hand by somebody who wants to say "the
files under `data/lang/`", and a pattern language with backtracking in it is a way for that person
to hang the tool on an archive with forty thousand entries in it.

## Switching a plugin's rule on makes it yours

A plugin's rules arrive switched off, whatever the file says - a rule changes how a game behaves,
and that decision belongs to the person building it. Switching one on copies it into the project's
own `rules/rules.json`, and from then on it is this project's rule: the plugin cannot switch it
back off, and updating the plugin will not change the rule somebody already accepted.

## Seeing what a plugin did

```
tjlocalizer plugins <project>
```

lists every plugin, what it claims, and whether each claim matched *this* game - a capability that
fired, a pattern that matched no file, a format this build does not have. The same appears in the
application's overview, and only when a project actually has plugins.

## What is deliberately absent

There is no plugin registry, no download, and no auto-update. A plugin is a file somebody put in
the project on purpose, which is the whole of its trust model.
