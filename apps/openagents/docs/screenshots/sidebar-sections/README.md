Native GPUI screenshots from the isolated sidebar fixture, with synthetic sessions
and no signed-in account or connected engine.

```sh
ZERON_SIDEBAR_COMPACT=1 cargo run -p zeron-ui --example sidebar-fixture --features project-palette-fixture
```

The sidebar's three-dot menu offers Create Section. Sections appear below Pinned
and above Sessions/project/device groups. Empty sections say “Drop sessions here”.
Section names, membership, and collapsed state now sync across devices within the
same account. Local workspaces retain device-local settings. These screenshots
show the unchanged section UI; registry tests cover sync and legacy migration.

Captured interactions:

- `view-menu.png`, `create-section.png`: create action and named-section dialog.
- `empty-section.png`, `sections.png`: empty and populated sections.
- `pinned-to-section.png`, `section-to-pinned.png`: moving the same session in both
  directions without duplicating it.
- `section-menu.png`, `edit-section.png`, `renamed-section.png`: hover menu and edit.
- `collapsed-section.png`: a hidden section body.
- `deleted-section.png`, `project-groups.png`: deleting a section returns its
  sessions to the normal groups; the empty remaining section stays above projects.

Validation: four section regression tests, all 33 existing pin/drag tests, settings
round-trip, native fixture build, changed-file formatting, and diff checks passed.
Section tests cover movement and mutual exclusion, persistence/profile isolation,
non-destructive deletion, every Archive all RPC (including a failed request), and
remote pin rejection/older acknowledgements. Archive all is tested with a mock
RPC connection; screenshots do not archive real sessions.
