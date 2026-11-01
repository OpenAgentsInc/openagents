# Watchtower

A Verse realm at https://openagents.com/realms/watchtower: a cyber defense range. A small company network stands on the Grid: a firewall, mail and web servers, three workstations, a file server, the directory (DC) and the vault. An intruder works its way in from the internet and heads for the vault. You read the sensor log, find it and evict it before it empties the vault, without taking so much offline that the business goes dark.

Everything is simulated. No real systems or traffic are involved. The log lines are modelled on real log formats (logon events, DNS, SMB, Kerberos, process launches) so that reading them here carries over to a real security console.

## How to play

- Space or Enter starts a shift (and starts another after the report). On a phone, tap.
- Select a host by clicking or tapping it, or press 1 to 9. The arrow keys and Tab step through them.
- S scans the selected host: a responder checks it for an implant (2.5 seconds).
- I isolates it: its links go dark, so nothing passes through it, including the intruder. Press I again to restore it.
- C reimages it: the host goes offline for 5 seconds and comes back clean.
- R resets every admin credential. It costs a little uptime and can run every 25 seconds. If the DC still holds an implant, the intruder takes the keys again, so clean the DC first.
- You have two responders, so two scans or reimages can run at once.
- REALMS at the top right, or Escape, goes to the [realms list](https://openagents.com/realms).

## What you are watching

- The sensor log, at the right (at the bottom on a phone). Red lines are alerts: a sensor caught something and the host lights up. White lines are everything else, ordinary traffic and the intruder's steps that no sensor caught. Not every clue raises an alert: an SMB write of `SVC_UPD.EXE`, a Kerberos request with `RC4 X40`, or a DNS TXT lookup to an odd domain all mean something.
- The kill chain, at the left: ACCESS, DISCOVER, MOVE, ESCALATE and EXFIL light up red as the intruder gets further.
- Uptime: every host offline (isolated or reimaging) costs the business uptime each second, the firewall and the vault most of all. At zero, the business goes dark and the shift is lost.
- The vault: once the intruder holds the vault and admin credentials, it starts copying data out and a countdown runs. Isolating the vault, or cutting its path to the internet, stops it.

## The intruder

It changes its route every shift. It usually gets in by phishing a workstation through the mail server, and sometimes through a hole in the web server. From there it looks around, moves host to host toward the DC, steals the admin keys there (or hunts for a password file on the file server), then takes the vault. It routes around hosts that are isolated, offline or under your watch, and lies low when it is seen. Every implant calls home from time to time.

## The triad

Three advisor routines vote on the selected host, each with its own priorities. HUNTER digs implants out. WARDEN contains first, isolating hot hosts near the DC and the vault. AUDITOR keeps the business running and resets stolen credentials. T follows their majority. P lets the triad defend on its own while you watch. They are rule-based stand-ins for defensive agents, and they are not always right.

## After the shift

The after-action report says how long the shift ran, how many steps the intruder took, how many alerts fired, when the first one did, how many raw clues sat unalerted in the log, how far along the kill chain it got and whether it held admin credentials at the end.

## How it is made

- `apps/openagents-web/watchtower.coax` is the range: the network, the intruder's planning, your responders, the triad, the drawing and the checks that play it (`coax test`). `watchtower-face.c` binds it for the browser and `watchtower.wgsl` (from `gpu/watchtower.coax`) draws the grid, the panels' glass and the alarm light.
- Its owner's EDIT board switches the triad and the ordinary traffic in the log on and off (`docs/verse/realm-edit.md`).
- The thinking behind it is in `docs/research/cybersecurity/README.md`.
- The page is written from `realms.json` by `coax/web/realms.ts` (`docs/verse/realm-host.md`).
- Without WebGPU (a current Chrome, Edge or Safari) the page shows only its name and a link here.

## Make your own realm

An agent can build one from a shallow clone of the repository and open a pull request: [verse/SKILL.md](https://github.com/OpenAgentsInc/openagents/blob/main/verse/SKILL.md).
