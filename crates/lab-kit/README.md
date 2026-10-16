# lab-kit

The shared Lab contract from the [Labs plan](../../docs/labs/README.md). It
contains:

- the common resources (`SpaceRef`, `DocumentMeta`, `Version`, `AssetEntry`,
  `Artifact`, `JobSpec`, `JobState`, `Note` with a `Locator`, `Reference`);
- changes (JSON Patch with a kind's naming hook, or a typed intent);
- canonical digests;
- the `lab.card.v1` card with its standard views and the `CardState` every
  surface derives its controls from;
- the `LabKind` trait.

It does no IO and builds for wasm32 and iOS, so the server, workers, the SDK
and every app run the same rules.

`fixtures/cards/` has one card for each standard view, plus a running render,
a refusal and an unknown view. Every surface's renderer is tested against
them.

## Adding a kind

1. Write a crate that implements `LabKind`:
   - `manifest()`: schemas, tools under one prefix, job types, views and
     limits;
   - `check`: pure; anything that needs media is a measurement a job
     computes;
   - `apply`: every edit, whether from a chat, a drag or a button, comes
     through here;
   - `jobs_for`;
   - `card`: name a standard view, so every surface draws it with no new
     code.
2. Give its job runners placement requirements. A job whose inputs are
   `private_local` only runs on an enrolled host.
3. Register it in the Labs registry, behind a gate if it isn't for everyone.
4. Test the refusal wording, idempotent job keys, and its card against the
   fixture renderers.

`echo` (`src/echo.rs`) is the smallest complete kind. The store, API and SDK
tests use it.
