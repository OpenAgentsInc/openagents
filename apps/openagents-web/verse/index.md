# Enter the Verse

The way into the Verse playtest at https://openagents.com/verse: CREATE ANYTHING, ENTER THE VERSE, and one line, "Edit our haunted house. Can you make it scarier?"

- **Your email, then GO.** The address is never verified (a format check only: no code, no password); it is kept in this browser so a returning visit goes straight on, and sent with this browser's anonymous id to `POST /api/playtest/join`, so the playtest can write to you when the Verse grows.
- **ENTER plays now as a guest.** Nothing is asked.

Both open the galaxy, [/verse/galaxy](https://openagents.com/verse/galaxy), where the Haunted House is the world that opens.

What you do here is recorded as the playtest recorded it, with an anonymous id: the visit, the page's load time, what you press, and which way you went in (`landing`, `load-landing`, `click`, `start`, `join`). Typed letters in the email box are never recorded as keys.

Drawn by Coax with WebGPU: `apps/openagents-web/verse.coax` (the page and what a press does), `verse-face.c` (its binding for the browser) and `verse.wgsl` (the stars and the plates). Without WebGPU (a current Chrome, Edge or Safari) the page shows only its name and a link here.
