# Coder / Verse on iOS (#11184), 2026-10-09

A debug simulator build (iPhone 17 Pro, not a preview build), from
`VerseShellUITests` with `OPENAGENTS_UITEST_APPEARANCE=light|dark`.

- `new-*.png`: a new chat. The top switch is **Coder** / **Verse**; the
  feature cards open on a random card (never the last one) and move on by
  themselves.
- `drawer-*.png`: the drawer with **Verse** in every build.
- `verse-*.png`: the drawer's Verse: the plain Grid with your avatar and the
  sticks. No Gym hall or boards and no Everglade arch outside a preview
  build. The world is dark in both themes, and so is its status bar.
- `try-verse-*.png`: **Explore the Verse**'s Try it, straight into the Grid.

- `enter-the-grid-*.png`: the live chat's answer to "What is the Verse?"
  (`meta.verse@1` from the deployed worker, release `a32a919471`) ends with
  the **Enter the Grid** card from its typed `verse` offer; a tap switches
  to the Grid.

The test then taps **Coder** and checks the chat is back with its composer.
