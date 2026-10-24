# Playable courtyard starter

Implemented for issue #1. The owned courtyard and Medieval Town use the same
Rust controller, first/third-person camera, generated/source environment,
proximity interaction, key-and-exit objective, and restart capabilities. This
small game does not require a behavior VM or edits to renderer internals.

## Run and play

Run `npm run build:kernel`, then `VITE_WORLD_PROFILE=starter npm run dev`.
The committed box courtyard, block character, and generated sky need no purchased
assets. Click **Play quest**, move with WASD, and use **Interact** or E near the
brass cube and then the green exit. V switches camera view. **Restart** clears
progress and returns to the starting pose; it preserves authored edits.

In the normal Medieval Town editor, **Play quest** places a barrel carrying the
key and a bench marking the exit ahead of the player. Both remain ordinary
editable objects. The same rules run over these objects; the private druid and
purchased town are optional scene content, not gameplay dependencies.

## Compose and edit

`packages/runtime/starter-game.ts` exports a versioned capability inventory,
validated `StarterGameConfig`, separate live progress, bounded proximity actions,
and deterministic accepted-event replay. `public/starter/game.json` composes the
capabilities into a complete game. Unsupported capabilities and invalid actions
fail without replacing working state. The trusted preview host supplies player
positions from Rust; project data cannot execute JavaScript or grant authority.

The semantic objects `starter-key` and `starter-exit` provide the interaction
positions. For example, call the existing editor command API:

```js
window.aethyrforge.command({
  kind: 'entity.transform', id: 'starter-key', position: [-1, 0, 1]
});
```

The same validated command path serves direct tools and agents. Rendering prepares
before the Rust transaction commits. Undo/redo moves the interaction with its
object. IndexedDB saves the authored markers; reopening reconstructs the scene
and starts fresh progress. Progress, inventory, and completion never enter that
save. Removing either marker stops the quest until its objects are restored.
The public fixture generator is `examples/starter/generate.py`.

Application code chooses objective placement and presentation. Reusable engine
code owns controller/cameras, capability validation, interactions, lifecycle,
commands, rendering, and persistence. The current rules run as bounded trusted
host code; this does not implement arbitrary creator scripts or multiplayer reward
authority. A published player cannot create missing quest markers.

## Verification

`npm test` covers range/invalid inputs, exact-once completion, reset, compatible
config edits, state restoration, replay, and owned asset validation. Run
`openagents browser run -- node tools/verify-starter.mjs` with `AF_CAPTURE_DIR`
pointing outside the checkout. Set `AF_VERIFY_WORLD=medieval-town` and
`AETHYRFORGE_PRIVATE_ASSETS` for the separate town run. The verifier records real
input movement separately from explicit camera positioning used to reach each
interaction, and checks persistence after a full reload. Captures of the town
remain private.
