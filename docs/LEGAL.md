# Attribution and legal boundary

Specification §26 and §36.

## What this tool asserts

Thanhtinz is credited for **the localization**: the translations, patches and assets Thanhtinz
created. Nothing else.

A build with branding enabled writes:

```
META-INF/THANHTINZ.BRAND
META-INF/LOCALIZATION.MF
```

containing the localization author, version and year. These are new files. The original
`META-INF/MANIFEST.MF` is not rewritten to add them.

## What this tool never asserts

Thanhtinz is **not** the owner or author of the original game. The tool is built so that claiming
otherwise takes deliberate effort:

- Original manifest attributes are preserved. `validate` raises an **error**, not a warning, if
  any original attribute was removed or changed - so a build that erased the original vendor or
  name fails and does not ship.
- Branding is additive and goes in its own files, so nothing about the original is displaced.
- `--no-branding` builds with no attribution at all, for cases where adding any would be wrong.

## Rights in the original game

This tool does not check, grant or infer any right to modify or redistribute a game. That is the
user's to establish before localizing anything.

`project.json` has an optional `permissionReference` field for recording a licence, permission or
purchase reference. It is free text: the tool records what the user asserts, and does not
adjudicate it.

## Third-party notices

Copyright and licence notices found in the original are preserved. Where a game's own terms
require notices to be carried into derivative works, that obligation is the user's, and the tool's
job is only to make sure the build does not silently drop them.
