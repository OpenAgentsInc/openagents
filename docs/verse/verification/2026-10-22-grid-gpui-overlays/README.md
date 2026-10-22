# No HTML over the Grid homepage (#12459)

Once Verse and the GPUI chat are up, nothing a visitor sees over the world on
`/` is HTML. Verse draws what belongs to the world, and the GPUI components
draw the rest, in the chat's visual language.

| Was HTML | Now |
| --- | --- |
| The top-center Game Lab button | Gone. Game Lab opens in the world: walk under the GAME LAB arch and press E, or tap the prompt there. Walking away, Escape, or Back to the Grid closes it. |
| Game Lab's panel and its card frame | A GPUI panel drawn by the chat: the sign-in line and Sign in for a signed-out visitor (to `/login`), or the visitor's version with Play, Compare with the original, the bot runs, and the version count. The game itself still runs in a frame the panel stands in its box. |
| Play's Back to Game Lab | A GPUI button over the game. |
| Kiln's panel frame | A GPUI header and border around Kiln's embedded page (`/admin/kiln/?embed=grid`, #12441), which draws its own translucent run panel. |
| The pylon card's panel and close button | The GPUI card draws its own panel and close mark. |
| The machines' labels in the Pylon Bay | Drawn by Verse on its canvas, from `globalThis.pylonsLabels`. The pylons layer still chooses what each says and keeps them apart. |
| The walking hint | Drawn by Verse at the bottom of the plaza view, with a touch wording on a phone. |
| The account bar | The GPUI top bar repeats the page's links; a click on it clicks them, so the visit's sign-in events still fire. |
| The chat's dark card | Gone: messages float over the world, each reply on a translucent plate, and New chat sits in the composer's row beside the model choice. |

The HTML stays in the page for screen readers, keyboards, crawlers, and
browsers that can't start Verse or GPUI. `#pylons-list` is still rendered by
the server and hidden once the pylons layer runs. The header, Game Lab's
section, and the play view's Back to Game Lab are clipped away and show again
only while they hold the keyboard's focus. The chat's canvas is clipped to
the boxes it reports drawing in, so the world under the rest still takes
clicks. GPUI still loads after Verse's first frame, as before.

## Captures

Taken by `scripts/dev/check-grid-home-overlays.py` under
`openagents browser run`, headless Chrome with WebGPU, against the
synthetic-account preview in
`crates/openagents-web/src/game_lab/hosted/tests.rs` (`browser_preview`,
with `OA_GAME_PREVIEW_RELAY=wss://relay.openagents.com` for the live pylons,
read only). It uses a scripted model and a synthetic account, never a real
one. Desktop is 1440 × 900; phone is 390 × 844 at 2× with touch.

| File | What it shows |
| --- | --- |
| `desktop-out-world.png`, `phone-out-world.png` | The world after joining: the GPUI top bar, Verse's labels and walking hint, and no HTML. |
| `*-out-pylon-selected.png` | A pylon clicked in the Bay: its GPUI card, and its label in full, drawn by Verse. |
| `*-out-game-lab-prompt.png` | Under the GAME LAB arch: "Press E to open Game Lab" ("Tap here to open Game Lab" on the phone). |
| `*-out-game-lab-panel.png` | After E, signed out: the GPUI panel with "Sign in to make your own version." and Sign in. |
| `*-in-game-lab-panel.png` | Signed in: the panel with the visitor's game, Play, and the chat's composer. |
| `*-in-game-lab-edited.png` | After "Make the man slower": version 2 with Compare, the transcript floating on plates with no card behind it, and New chat in the composer's row. On a wide page the chat moves left of the panel. |
| `*-in-game-lab-play.png` | Play: the game full screen with the GPUI Back to Game Lab. |

Each run's `*.json` holds the page's performance marks, what the chat
reported drawing (`overlay_*`), and the visit's events. `html_seen*` lists
every element with a painted box or visible text over the world that isn't
a canvas, hidden, clipped away, or a framed page: it is empty in all four
runs, at spawn, with a pylon selected, and with Game Lab open. The visit has
`game_lab_opened` and `game_lab_closed`, and, signed in, `game_lab_check` and
`game_lab_played`. The desktop run also has `game_lab_version_saved`; the
phone run, on the same synthetic account, asked for the same change, which
saved no new version.
