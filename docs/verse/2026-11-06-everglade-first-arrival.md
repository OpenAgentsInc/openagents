# Everglade: the first two minutes

Date: 2026-11-06. Status: proposed experience specification; not an implementation claim.

Everglade is the first world reached from the Grid. A first-time visitor creates a
character before entering, arrives as that character, and discovers a village being
attacked by cultists. In roughly two minutes of active play, they should understand
where the danger is and make one visible contribution to its defense. The timing
below is a pacing target, not a countdown or a requirement to rush.

This opening builds on the [world-events and Cultist Raid spec](2026-11-03-everglade-world-events.md),
the [multiplayer and Grid hand-off](2026-11-03-everglade-multiplayer-spec.md), and
[Character Studio](character-creator.md). Those documents describe existing systems
and their own status. This document proposes the experience that joins them; it does
not assert that the joining work, dialogue, prompts, or tracking are already built.

## The promise

The player has just made someone they want to be. Let them see that person in a
place worth protecting, discover an immediate threat, and help the people already
responding. The world should feel active without waiting for the player, but their
action should have an observable effect. The opening must work alone and in a room
with other players, on desktop and touch, without sound.

## From character creation to the village

The Grid's Everglade portal leads a first-time visitor to character creation before
joining the world. **Enter Everglade** accepts the current appearance, including a
default if the player makes no changes. The selected appearance must carry into the
actual player character, not just the preview. If saving or transferring the look
fails, explain the failure and offer retry; never silently enter as a different
character. A returning player with a valid look goes straight in and can edit it
later. The current Studio's local CC2 save is an existing piece, not yet proof of
cross-page or account persistence. Guest and account persistence and the precise
entry integration need implementation and verification.

After the player enters a room, the existing Grid hand-off's brief veil lifts. The
camera settles behind the character with their appearance visible, on safe ground
inside or near the South Gate, oriented so the route to the defense is legible. The
current hand-off specifies a south-road spawn facing inward; selecting an exact safe
arrival position and camera is work for this opening, coordinated with the room's
spawn and raid rather than a client-only relocation. Keep the player in control; no
unskippable camera flight or forced movement.

## The experience

| Active time | What the player sees and does | Guidance and response |
| --- | --- | --- |
| 0–15 s | The veil clears. The player sees their character, the village, and guards moving toward the South Gate. A horn, visible signal, smoke or spell flashes point toward the danger. | Show one device-appropriate movement hint, such as **Move to follow the guards**. The signal also has a visual equivalent. The spawn remains a safe place to look around. |
| 15–35 s | Moving toward the gate reveals the watch fighting robed cultists on the road. A mayor or guard addresses the player from a safe, reachable position. | Show a short caption: **“Cultists are attacking the South Gate. Our guards need help!”** Optional voice reinforces the caption. Dialogue never blocks movement or requires a click to dismiss. |
| 35–75 s | The player reaches the edge of the fighting. A nearby cultist presents a clear first target; the player uses their available basic attack or spell. | Show the actual bound control for the player's device and current ability: **Use [control] to attack a cultist**. Point out a valid target without forcing aim or taking control. A hit has a visible impact and the enemy reacts; the guard continues fighting. |
| 75–120 s | The player can keep helping the watch. A brief acknowledgment follows their first confirmed contribution, then the player is free to continue or explore. | **“You helped defend Everglade.”** Explain the next immediate goal in plain words: **Help the watch push the cultists back.** If healing or repair is available and applicable, it may also count as help. |

These are proposed words, not final recorded dialogue. The existing
[mayor's raid promo cue](../../verse/cast/cues/raid-promo.cues) supplies an established
voice and premise. Introduce Darian Odiodei after the immediate request, for example
when the player sees the camp: **“Their leader is behind those wards.”** Do not
require a new player to memorize his name or understand the ward mechanics first.
A distant glimpse of the leader and his camp should suggest a larger story, while
the nearby cultist is a comprehensible first challenge.

## Interaction and event rules

- **Real participation.** The first contribution is a room-confirmed successful
  damaging hit on a raid enemy, effective healing of a defender or player, or an
  effective town repair, where those actions are actually available. An attempted
  cast, a miss, overhealing, or an ineffective repair does not claim success. Use
  the existing event's server-authoritative outcomes; do not award a scripted kill
  or change the front solely because a prompt appeared. No new XP or reward is
  promised by this opening.
- **Clear feedback.** Acknowledge the player's own confirmed contribution once,
  even if another defender finishes the enemy. Do not claim the player saved a
  particular guard or won the raid unless that actually happened. Battle effects
  remain readable alongside the player's target and control hint.
- **Safe learning.** Preserve the raid's protected spawn. Do not require a player
  to leave it until they choose to; show the direction of the fight from there.
  Once they leave, ordinary combat rules apply. If they fall, explain the existing
  10-second return and let them try again; do not reset their appearance or opening
  progress. Do not force them to attack a friendly character or Darian.
- **Prompts respond to actions, not a timer.** Advance a hint when its action occurs;
  avoid repeating it after success. If the player waits, keep a quiet, dismissible
  direction available. If they explore, they can return to the defense later. Never
  teleport, lock movement, or fail the opening because two minutes have elapsed.
- **Shared world.** Do not restart, pause, or secretly reshape the room's raid for
  each arrival. Other players and the watch fight in the same authoritative event.
  A newcomer joins the current phase; their own guidance and acknowledgment are
  local to them and must not interrupt anyone else.
- **Event availability.** When the raid is inactive or has just ended, do not claim
  the gate is currently under attack or wait indefinitely for an admin. The launch
  configuration should make an active siege available for a new arrival, subject to
  room capacity and friends joining together. If no active siege can be offered,
  enter the village safely, say the watch is preparing for another attack, and give
  the player a clear route to the gate and an optional way to join the next event.
  Do not fabricate combat or auto-start an event for an occupied room. Exact room
  selection and event scheduling are implementation decisions to validate.
- **Already in progress.** If the front has moved toward the camp, direct the
  newcomer toward the actual active front, not an empty gate. If the camp has fallen,
  use the inactive/aftermath path; do not promise a live target. A solo player can
  contribute and receive acknowledgment without needing to win the whole siege.
- **Access.** Captions and visual threat cues carry the essential information without
  audio. Hints use actual desktop or touch bindings and remain readable on a phone;
  they do not cover the character, target, health, or essential controls. Voice is
  optional and must follow the browser's audio-gesture rules.

## After the first contribution

The acknowledgment is the end of the opening, not the end of the event. Leave the
player in the world with control. They can continue fighting, see the battle line
move with sustained help, or walk back into the village. If implemented and relevant,
show healing or repair as additional ways to help. Avoid promising a quest, persistent
world changes, or an individual victory screen before those systems exist.

## Delivery and acceptance

Build the smallest end-to-end slice first: portal → character acceptance and look
handoff → safe arrival → contextual movement and attack hints → a real server-confirmed
contribution → acknowledgment. Integrate the existing raid and its controls; the
opening does not change the raid's event lifecycle or published protocol by itself.
If new wire data is needed, specify it through the protocol process before changing
the contract. The final spawn, dialogue presentation, current starter ability, and
active-event routing must be checked against the running build before implementation.

Acceptance for the launch slice:

1. A new guest can accept a default or customized look, enter from the Grid, and see
   that same look on their player. A failed handoff has a visible retry path.
2. In an active raid, a player can identify the South Gate and perform a real useful
   action without outside instruction. A representative first-time playtest should
   aim for this within two minutes of arrival; observe actual times and confusion
   before treating that target as achieved.
3. The acknowledgment occurs once after a confirmed effective contribution and never
   for a miss or an ineffective action. A solo player can reach it without victory.
4. Spawn safety, death and return, late joining, an inactive event, and an event
   ending during the opening all leave the player able to proceed without misleading
   instructions. One arrival does not disrupt other occupants.
5. Check desktop and phone layouts and controls, including muted play, with actual
   participants. Record where they stop, what they believe the objective is, time
   to first contribution, and whether they choose to continue. This is a playtest
   target, not evidence that current implementation passes.

Open decisions: the final spawn and sightline; whether the speaker is the mayor or a
watch captain; the starter class and basic action; which noncombat contributions are
ready on day one; and how launch traffic is routed to an active siege without
splitting friends. Test those decisions in the live experience before fixing copy or
cinematics around them.
