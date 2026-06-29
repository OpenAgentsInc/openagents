# Effect-Driven Demo Recording Notes

Route: `/demo`

Scope: local or staging review only until the fixture data is approved for a
public recording window.

The demo route uses synthetic OpenAgents Core Team, Artanis project, run, file,
and message fixtures. It does not call live Autopilot launch, provider account,
billing, sync stream, thread-file, R2, D1, GitHub, or admin APIs. The wrapper
drives the nested logged-in workroom model with demo cues and discards every
command returned by the nested logged-in update function.

Recording flow:

1. Start the app locally with the normal web dev command.
2. Open `/demo` in a desktop recording viewport.
3. Reload before recording to restart the deterministic 15-second sequence.
4. Stop on the file detail state after the reference list is visible.

Safe fixture update rules:

- Keep all demo records synthetic and Schema-backed in `apps/web/src/page/demo/fixtures.ts`.
- Keep all playback cue changes in `apps/web/src/page/demo/playback.ts`.
- Do not link `/demo` from normal product navigation without an explicit review.
- Do not replace the nested logged-in workroom path with screenshots or static mock markup.
