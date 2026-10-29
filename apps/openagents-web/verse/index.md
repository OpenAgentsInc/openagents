# Enter the Verse

The way into the Verse playtest at https://openagents.com/verse: A HAUNTED HOUSE PLAYTEST, ENTER THE VERSE, one line, "Explore our haunted house. Can you make it scarier?", and **PLAYTEST NOW**, which opens the Haunted House directly, [/realms/haunted-house](https://openagents.com/realms/haunted-house). The galaxy of worlds is still at [/verse/galaxy](https://openagents.com/verse/galaxy). Nothing is asked.

In the house there is one camera, behind you, and only your name, the minimap, a voice button and the hotbar on screen: take a thing from the hotbar and click or tap where it goes ([the realm's page](../realms/haunted-house/index.md) has the controls).

Under the button is the Agent orb, the playtest's guide. Hold it and talk: it answers out loud what the playtest is for, what the Verse is and what the Haunted House is, and its words show under it. The microphone is on only while you hold it (push to talk, `coax/web/voice.js`, the voice host's `guide` route in `verse/voice-host/speech.c`).

Behind it, faint grey worlds pull on each other. Press and hold in the stars to make one: it grows while held, joins the rest when let go, bounces once off another and then merges with it in a burst of sparks.

What you do here is recorded as the playtest recorded it, with an anonymous id: the visit, the page's load time, what you press, and which way you went in (`landing`, `load-landing`, `click`, `start`), and that you talked to the guide (`voice-on`, never the words). Someone who gave the old playtest an email in this browser is joined again under it (`join`).

Drawn by Coax with WebGPU: `apps/openagents-web/verse.coax` (the page, its orbs and what a press does), `verse-face.c` (its binding for the browser) and `verse.wgsl` (the playtest's field of stars, the PLAYTEST NOW glass and the plates). Without WebGPU (a current Chrome, Edge or Safari) the page shows only its name and a link here.
