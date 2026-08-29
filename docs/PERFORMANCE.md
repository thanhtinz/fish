# What it costs to run

The numbers that matter for this tool are the ones a person waits through: opening a game,
extracting its text, proposing translations for all of it, building, validating.

```
tools/bench.sh 600 200
```

builds a synthetic game - 663 entries, 240,429 strings - and times each step. Synthetic rather
than a real game because a real one cannot be committed, and a benchmark nobody can run is a
number nobody can check.

## Where it stands

On the machine this was last measured on, a release build:

| Step | 40,149 strings | 240,429 strings |
| --- | --- | --- |
| Read the archive | 2 ms | 14 ms |
| Extract | 63 ms | 412 ms |
| Read the context | 9 ms | 70 ms |
| Propose every line | 303 ms | 1,963 ms |
| Build | 33 ms | 167 ms |
| Validate | 4 ms | 26 ms |

Six times the text costs about six times the time in every step, which is the property being
checked. A J2ME game has a few thousand strings; the largest column above is far past anything
this tool will meet, and it is there so that a change making one of these quadratic shows up as a
number rather than as a complaint months later.

## The one that was slow

`propose every line` took **5.6 seconds** for 40,149 strings before the dictionary was indexed:
18 times what it takes now.

The cause was the difference between how the dictionary was written and how it is used. Written
for "look up one word", it walked every entry of every pack for every question - and the segmenter
did worse, folding and grouping all six hundred entries, then re-splitting each term into
characters, at every character position of every line. Six hundred entries against forty thousand
lines is a wait somebody notices.

The fix is not clever: the entries are arranged once, into a map from folded term to entries and a
list of terms longest-first with their characters already split out, and thrown away when a pack is
added. The order entries were listed in is preserved, because a tie between two readings goes to
the first and that is a promise the dictionary makes.

## What has not been profiled

Memory. The whole archive, the whole graph and every translation are held at once, which is
comfortable for a J2ME game and untested for a directory-shaped PC game of several gigabytes -
`tree` reads those selectively for exactly that reason, but nothing here has measured what it
costs when a person points it at one.
