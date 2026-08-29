# What a line is for, and who says it

`graph::classify` looks at one string on its own. That is enough for a path, a format string or a
sentence, and it is not enough for the strings a game is mostly made of:

* `Yes` is two characters with no punctuation, so it is a button. It is also half the answers in
  any conversation.
* `Iron Sword` is a short noun phrase, which is an interface label, and an item name, and a quest
  objective.
* `Blacksmith: I can mend that` is a sentence, and the first word is not part of it.

What settles those is the company the string keeps, and that is what `context` reads.

## The three things it looks at

**A named speaker.** `Name: line` is how a very large share of games write down speech. The test is
strict on purpose - a colon appears in `HP: 20`, in `Time: 3:00`, in `Score: %d` and in a URL, and
reading any of those as a character would put a fictional cast in front of a translator. The name
must be short, capitalised, free of digits and punctuation; what follows must be more than one word
and must hold no placeholder.

**A section of keys.** Keys under one prefix are a section, and a section is usually one kind of
thing. Where the prefix is a word games agree on - `menu`, `dialog`, `quest`, `item`, `skill`,
`tutorial` - that word says what the section is. Where it is not, what the section's own
confidently-classified members are says it instead, by majority. Three keys minimum: two keys are
not a section, and a rule that fired on them would be guessing dressed as inference.

**Neighbours in a constant pool.** Strings arrive in the order the compiler wrote them, which
follows the order they appear in the source: a screen's labels are together, a conversation's lines
are together. A string with nothing to say for itself, between two that agree, takes their reading.
Both sides, and agreeing: one neighbour is a coincidence.

## What it will not reconsider

A string that settled its own case. A path is technical, a format string is a format string, a
sentence is a sentence - those are findings about the text, and the text is the thing being
translated.

What it may reconsider is the reading a string got *for being short*. `Ui` on a two-character
string says "this is short", which is a description of its length rather than a finding about its
use, and it is exactly the case the surroundings can answer.

## The cast

Every named speaker becomes a character: how many lines, which files, and which other characters
are named in the same file. That last one is co-occurrence and is called co-occurrence - who is
whose brother is in the game's story, not in its strings, and a relationship graph asserting
otherwise would be fiction with a schema.

Each character also gets a stance their lines lean towards, with the words that leaned that way
listed verbatim. **Nothing applies it.** Vietnamese has no neutral second person, so choosing
between `ngươi` and `bạn` for a character is a decision that then governs every line they speak -
and a decision inferred from the word "please" is not a decision. It is offered, with its evidence,
for a person to accept or ignore.

## Where it lands

Extraction runs it and writes `content/context.json` beside the graph. Nothing in the graph is
changed: the readings sit beside it and carry their evidence.

The inferred speaker reaches every translation request, which is the point of the whole module -
`provider` turns a speaker and a stance into the pronouns an engine is told to use, and a line a
character says, translated as interface text, addresses the player as nobody.

```
tjlocalizer context <project>          # every reading, with why
tjlocalizer context <project> --cast   # just the characters
```

The application shows the cast on the overview, and uses the speaker automatically when proposing.
