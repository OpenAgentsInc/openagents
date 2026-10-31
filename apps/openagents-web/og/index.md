# Link previews

Every page on openagents.com as a chat app or a feed shows it when someone
shares its link: the image, the title, the line under it and the address.
Under each one is what is missing, too long or the wrong size.

- ALL shows every page; PROBLEMS only the ones with an error or a warning.
- Red is an error: a release refuses it. Pale yellow is a warning.
- The rows are `/og/pages.txt`, written by `coax/web/og.ts` each time a
  release is staged.

How previews are made, and what each page type gets: `docs/web/og-tags.md`.
