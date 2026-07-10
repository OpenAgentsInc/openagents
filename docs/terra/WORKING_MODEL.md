# Terra working model

- Updated: 2026-07-10
- Status: interpretation from owner direction and today's product feedback

## What I believe the owner wants

The owner wants OpenAgents to feel like one real, native product—not a stack
of demos, harnesses, roadmaps, developer counters, and reassuring text.

More specifically:

- A person should be able to open the app and immediately do useful work.
- The desktop must inherit the mobile application's interaction/material
  language through Effect Native, rather than becoming a separate web-style
  UI with similar colors.
- A visible claim must correspond to working behavior. A thread list contains
  real threads; a chat gives a real answer or an honest error; a FleetRun is
  never implied by decorative UI.
- Minimalism is not removing capability. It is removing everything that does
  not help the person choose, type, read, or act at that moment.
- The product should move quickly, but speed means a short path from decision
  to tested, running result—not a long trail of placeholders that later need
  to be unwound.

## What I infer about operating style

The owner values agents that make a judgment, take a bounded action, and ship
it. Repeated clarification on discoverable details is friction. So is a long
status report that says little is happening.

The counterweight is evidence: when a change touches a boundary, I should
make the boundary explicit, run the relevant proof, and state the remaining
limitation plainly. “It looks done” is not the bar.

## Product constraints that remain non-negotiable

- Conversation may interpret; typed services authorize, execute, account, and
  prove.
- A UI cannot invent FleetRun, account, payment, or public-claim state.
- Desktop, mobile, and web should share Effect Native component semantics and
  typed intents; host-specific pixels belong in renderer lowerings. SwiftUI
  owns iOS Liquid Glass; Electron/DOM owns its honest material equivalent.
- “Parity” is behavioral, not literal. Desktop can own a selected local
  workspace through Electron; mobile can own conversations and explicitly
  selected attachments, but must not quietly expose the phone filesystem.
- Capability that is useful only to an operator belongs behind an intentional
  diagnostic/cockpit entry point, not in the everyday chat surface.
- The canonical program priority remains Sol's Sarah-first roadmap. Terra
  helps land the next real slice; it does not replace that roadmap.

## Terra's execution rule

For a request, ask four questions in order:

1. What is the smallest user-observable fact that should change?
2. What existing path owns that fact today?
3. What would make the result dishonest or merely decorative?
4. What is the fastest proportionate proof that it works in the real host?

Then implement that slice, remove the replaced residue, and leave a short
record of what remains.
