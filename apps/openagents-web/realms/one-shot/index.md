# One-Shot

A Verse world at https://openagents.com/worlds/one-shot, not listed on the worlds page. A Game Master (GM) makes a short 5E adventure, runs it for friends from the GM Screen, and each player plays a hero on their own device. Every roll, attack and hit point comes from the shared SRD 5e rules engine. It is the first phase of the campaign creator in `docs/dnd/2026-11-03-dnd-campaign-creator-spec.md`.

## Run a game as the GM

1. On **Campaigns**, press **NEW ONE-SHOT**. You get a copy of the starter adventure, "The Bell in Mossgrave Glade": three scenes for heroes at level 3, two of them with Skeletons to fight.
2. In the **Campaign Builder**, change what you like: the title, the heroes' level (1 to 6), and for each scene its name, its read-aloud text, its handout, its art (a title card or the 3D glade) and how many Skeletons it has (0 to 8). You can add a scene (up to four) or remove one. Your campaigns are kept in this browser.
3. Press **RUN IT AT THE TABLE** (or **RUN IT** on Campaigns). The GM Screen opens with a five-character table code.
4. Press **COPY INVITE LINK** and send the link to your players, or read them the code.
5. On the GM Screen:
   - Under **SCENES**, press **RUN IT** on a scene. **SHOW TO TABLE** puts it on the Table Display, **SHOW TEXT** adds its read-aloud text, and **SEND HANDOUT** shows its handout to everyone.
   - **START COMBAT** puts the scene's Skeletons on the grid and rolls initiative for every creature. The tracker lists them in initiative order with their hit points. Tap a row to pick a target, then press an ability of the creature whose turn it is. Heroes' players can take their own turns from My Character. **NEXT TURN** (or N) moves the turn on.
   - The **DICE** tray rolls a d4 to a d100. **BEHIND THE SCREEN** keeps your rolls hidden; **OPEN TO THE TABLE** shows them. **PHYSICAL DICE** lets you roll real dice and enter the face.
   - **MONSTER ROLLS: HIDDEN** keeps the Skeletons' rolls out of the players' log. Each hidden line in the **SESSION LOG** has a **SHOW** button that reveals it.

## Join as a player

- Open the GM's invite link, or enter the code under **JOIN A GAME** on Campaigns.
- On **My Character**, pick a ready hero (one of each of the twelve classes), or make a quick build: a class, a name and a level from 1 to 6. Press **JOIN THE TABLE**. The GM's page seats you.
- Your sheet shows what the rules engine computes for your hero: hit points, Armor Class, Speed, ability scores and saves, your weapon, and your spell attack, save DC and slots.
- On your turn, pick a target, then an ability on your hotbar. **END MY TURN** passes the turn. The dice tray rolls in the open.
- The **TABLE DISPLAY** tab shows what everyone sees. Drag to turn the 3D glade, and use the wheel to zoom.

## Show the table on a TV

Open `https://openagents.com/worlds/one-shot?display=CODE` with the table code on the shared screen. It shows the scene, the initiative order, the latest roll and the handout, with no controls.

## One device

If other devices can't reach the table, everyone plays on the GM's device. On **My Character**, **ADD A HERO** seats another hero, and the hero buttons along the top switch between the seats.

## Limits in this first phase

- The table holds the GM and three other screens (players or a display).
- Encounters use the SRD's Skeleton, the only monster stat block the rules engine has today.
- Dice come from the table's own seeded stream. Provably fair dice are a later phase.
- Heroes and the table's state last as long as the GM's page is open. Only the campaigns are kept.

## How it is made

- `apps/openagents-web/oneshot-table.coax` holds the campaigns and the table: the starter adventure, the builder's edits, joining, initiative and turns on the SRD engine (`aethyr/rules/srd/`), the dice, the session log with its hidden lines, and the frames sent between pages. Its checks cover each of them.
- `apps/openagents-web/oneshot.coax` draws the five screens and carries the table between pages through the room server's relay (`coax/web/room/room-relay.coax`). The GM's page decides everything; the other pages send what their player asked for. `oneshot-face.c` binds it for the browser.
- `oneshot.wgsl` (from `gpu/oneshot.coax`) reuses 5E Lab's glade shader (`gpu/fivelab.coax`) for the 3D scene.
- Campaigns you write are your own content. They are kept apart from the SRD and saved with `srd: false`.

This work includes material from the System Reference Document 5.2.1 ("SRD 5.2.1") by Wizards of the Coast LLC, available at https://www.dndbeyond.com/srd. The SRD 5.2.1 is licensed under the Creative Commons Attribution 4.0 International License, available at https://creativecommons.org/licenses/by/4.0/legalcode.
