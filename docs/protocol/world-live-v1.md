# Dev and live: a world's two revisions, and when an update reaches a session

Issue: [#13412](https://github.com/OpenAgentsInc/openagents/issues/13412).
It builds on [the release set](../deployment/release-set.md) (#13299) and
[the previous pins](everglade-previous-pins.md) (#13408). The live example is
#13397: production's rooms spoke wire 6 while the page spoke wire 5.
Code: `metaverse-engine/editor/world-live.coax` (the rule, `#authority`),
`coax/web/api/world-policy.coax` (the API's use of it).

## Two revisions

| Lane | Who joins | What changes it |
| --- | --- | --- |
| `dev` | People and agents who edit (`?edit`, the CLI) | Every saved edit |
| `live` | People who play | A publish, which copies dev |

Edits never reach people who play until someone publishes.

## A publish says when

A publish is a batch with `do` set to `publish-now` or `publish-next`
([edits](world-edit-v1.md)). It names the dev revision it read.

- **now:** open sessions take the update at once.
- **next:** open sessions keep what they joined. The update starts with each
  person's next session.

## The pins decide

A live revision carries the pins of the build that reads it: the engine, the
rules, the content tables' revision and the wire. The four are matched
together.

| Rule | Why |
| --- | --- |
| A session never takes a revision whose pins are not its own | A page can't read a document made for another build |
| An update that changes a pin can only be published for the next session. `publish-now` is refused and says so | Open pages are the old build until they reload |
| Live keeps the revision before the last pin change, for one release | A release moves its parts in order, and a page a person has open is the previous release's. That page is served the revision it reads |
| A session on any other pins is told to reload (409) | Two releases behind is past the overlap |
| A publish that changes no pin leaves the kept revision as it is | A repair must not lose the revision an older page reads |
| One raise a release | A second pin change replaces the kept revision |

## What a page does

1. It reads live with its pins: `GET /api/worlds/ID?lane=live&engine=E&rules=R&content=C&wire=W`.
2. It follows the live lane's events. For each
   `realm-authoring.world.published.v1` it asks `wl_follow`: it takes the
   update now only when the publish said `now` and the pins are its own.
3. Otherwise it keeps what it joined until the next session.

## Checks

- `coax/bin/coax test metaverse-engine/editor/world-live.coax --library`, and
  `coax mutate` with zero survivors.
- `coax/web/api/tests/world_checks.coax`: a pin change refused for now and
  taken for next; the revision before it kept and served; a repair that
  leaves it.
