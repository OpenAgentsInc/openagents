# Setting up a project, and creating a realm from a drop

How a project such as Coax, Verse or Planetary Villa comes to exist on
openagents.com/projects. Part 1 is the checklist for doing it by hand, as we do
while bootstrapping, and it serves as the template for future project setups.
Part 2 is where this is going: drop in your assets, images and documents, and
the site turns them into a complete project card and its realm. As each step
moves onto the website, it comes out of Part 1.

## Part 1: by hand, today

| # | Step | Where | Notes |
| --- | --- | --- | --- |
| 1 | Pick the slug, name, owner and visibility (public, unlisted or private) | — | The slug is lowercase with dashes (`planetary-villa`) and never changes. A private project follows a separate path, at the end of this table |
| 2 | Make the project's key | `~/work/.secrets/openagents-project-<slug>.key` | 32 random bytes as hex, chmod 600, never printed or committed. The publisher signs everything with it |
| 3 | Write `project.json` | `apps/openagents-web/projects/<slug>/project.json` | slug, name, one-line description, relay, repo, path (the repo folder whose commits the page draws), contributions, licence, pubkey (the key's public key), owner. Copy `projects/verse/project.json` |
| 4 | Write the design document | `<path>/README.md` (for example `verse/planetary-villa/README.md`) | The owner's own words, with open questions for them. Agents build from it. A project's folder goes under the area it belongs to (a Verse world under `verse/`), never at the repository root |
| 5 | Write the roadmap as a work seed | `projects/<slug>/work/<initiative>.json` | Schema in `projects/coax/work/README.md`: one initiative, milestones, tasks with acceptance checks, sizes, files, `blocked_by`. Every mechanic in the design doc is a task |
| 6 | Link work another project must do first | both projects' work seeds | The waiting task lists `needs: ["<project>:<slug>"]` and says so in its body. The other project's task says who needs it. See below |
| 7 | Publish to Nostr | `cd coax/web/project && bun publish.ts publish --project <slug>` | Publishes the project (kind 30621), its issues (1621) and their order (30623). Use `--dry-run` first |
| 8 | Add the page | `apps/openagents-web/projects/<slug>/index.html` | Copy `projects/verse/index.html`; change the title, the description and `data-text` to `"<pubkey> <slug>"`. That's all: the build finds every `project.json` beside an `index.html`, snapshots it from the relay, bakes its commits (from `path`) and its owner, and the shared project app draws `/projects/<slug>` (`coax/web/project/baked.ts`). No Coax or build edits |
| 9 | Smoke it | `coax/web/smoke.sh` | Nothing to add: the smoke checks `/projects/<slug>` and its `index.md` for every page the checkout has. Check the build locally with `apps/openagents-web/build.sh` |
| 10 | Release | `coax/web/release.sh` | Staging, smoke, production, smoke. The project appears on `/projects` and its page goes live |
| 11 | Tell the owner how to work it | `coax/web/project/QUEUE.md` | Workers claim, plan, note and finish tasks with `publish.ts`, and the page shows it live |
| 12 | Hand ownership to the owner's account | `project.json` `owner_pubkey` | Once the owner has a Nostr identity (phone sign-in), their key becomes the project's maintainer |

**Private projects** skip steps 7–10. Their contents never go on the public
relay, into the public bucket or into this repository. They live in a private
repository and are served only to members by `coax/web/api` (private
projects). The page route is added with a command whose argument comes from
outside this repository.

### Work one project waits on from another

When a task needs something another project builds (Planetary Villa needs
moonlight, particles and four-legged characters from the Verse engine):

- **Make the work in the project that owns it.** For engine upgrades, that
  is the Verse project's `verse-engine` initiative. Reuse an existing task
  when one already covers it, such as the playtest's saves or presence.
- **The waiting task names it** in `needs` (`"verse:verse-particles"`), is
  labelled `needs-verse`, and its body says it can't land until that's done.
- **The owning task names who needs it** ("Needed by Planetary Villa:
  hc-realm-particles") and is labelled with the waiting project.
- **Both pages show the link.** The waiting task stays blocked until the other
  task is done.

## Part 2: create a realm from a drop

The aim is to manage everything on the website.

1. **Drop.** On `/projects`, NEW PROJECT opens a drop area. Drop in anything:
   - design notes;
   - a voice memo or its transcript;
   - concept art, screenshots and reference images;
   - 3D models, textures and sounds;
   - existing documents.
2. **Read.** An agent reads the drop. Model calls use our Google-first
   routing. From the drop, the agent writes:
   - the project card: name, one line, the owner, a cover image picked or made
     from the drop, and the art direction;
   - the design document, in the owner's words, with its open questions;
   - the roadmap as milestones and tasks with acceptance checks;
   - the asset inventory: what was dropped, its licence and what still has to
     be made;
   - every engine upgrade the design needs, as linked tasks in the Verse
     project;
   - a first realm: the drop's assets placed in an empty realm, so there is
     something to walk around on day one. Built: on PUBLISH every dropped
     `.glb` is packed (`coax/web/api/drops.c`, `0040_project_realm.sql`) and
     stood on a floor at `/projects/<slug>/realm` (`droprealm.coax`), which
     the project's page header (WALK THE REALM) and its `index.md` link.
3. **Review.** The owner sees the card, design, roadmap and links as a draft,
   corrects any of it, and publishes. Nothing goes public until then.
4. **Publish.** The site makes the project's key (held for the owner until
   their account takes it), publishes the project and its work to Nostr,
   stores the assets, and lists the project on `/projects` with its page and
   realm live.
5. **Grow.** Later drops on a project's page add to it: new assets, new
   design notes, new milestones. Each drop is turned into proposed changes
   the owner accepts.

The steps to build Part 2 are tasks in the Verse project under "Realm
creation from a drop" (https://openagents.com/projects/verse).
