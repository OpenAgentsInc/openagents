# Planetary Villa

Owner: Kiki. Project page: https://openagents.com/projects/planetary-villa.
This is Kiki's design, written down on October 28. She corrects it; agents build
from it.

## The world

Planetary Villa is a world on the Verse with game expansions off it. You find it
as a planet on the galaxy map. Choosing it plays a cinematic of traveling to the
expansion this session focuses on.

## The first expansion: Hecate's Cemetery

- **What it is:** a 3D game with the same perspective as *The Legend of Zelda:
  Breath of the Wild*.
- **Look:** a simple stylized, almost low-poly art style. Everything is lit by
  moonlight, so it's not very bright, with some outlines and garden paths.
- **The moon:** it doesn't move in the sky. Its phase is the real moon's phase
  right now.
- **Make your pet:** you upload data about your pet: a late pet, or a new one.
  - Text, images or video help the agent build the avatar.
  - The photos and videos are kept, for the pet's memorial page.
  - You customize the avatar the way you would in the Sims.
- **Emotes:** there's a set to choose from, and the agent can make new ones
  from what you say about the creature.
- **Skills:** fetch, obedience, digging holes, and room for more.
- **Play:** you press play and you're in a cemetery that is also a beautiful
  garden.
  - Tombstones represent each pet's existence in the world.
  - Each tombstone shows the pet's achievements (if it was competitive) and its
    most beloved traits and stories.
- **Memorial page:** each tombstone links to a testimonial-style memorial page
  for the pet. The memorial page has a shareable link.
  - Every memorial uses the same clearly formatted template. It puts first
    what was uploaded when the pet was made, and more can be added later.
  - Anyone can write a comment on a memorial page.
- **Multiplayer:** the world is multiplayer, with a chat box like Horse Isle's.
- **What's in it for now:** mini games come later. For now there is the
  cemetery, plus an attached open field where you play fetch with your pet and
  explore other tombstones and memorial links.
- **Pets:** dogs first; other animals come later. The canine characters glow
  white.
- **You:** your character doesn't have many details yet. It's mostly a shadow:
  an all-black humanoid with fuzzy edges, and maybe a particle system to show
  you're visiting from another realm.

## The glowing flowers mini game

- **Collect:** pick glowing flowers of three kinds in the field.
- **Arrange:** in a close-up view, arrange them in a vase by pointing and
  clicking.
- **Place:** put the vase anywhere in the world. The tutorial directs you to
  place it by the tombstone.
- **Stay:** a placed vase stays until someone cleans it up. Its flowers decay
  over time.

## The details the tasks build to

Each mechanic above, as the project's tasks build it. Kiki's words come first;
where these details go further than she said, she corrects them.

- **Arrival:** Planetary Villa is a planet on the galaxy map. Choosing it plays
  a short flight through space to Hecate's Cemetery. You can skip it, and it
  ends at the garden's gate with control handed to you.
- **The garden:** paths between tombstone plots, with room for at least 64
  tombstones (more as pets arrive), hedges, trees, benches, a fountain and a
  gate into the open field. Every path is walkable.
- **The art kit:** a small set of low-poly models in one look: tombstones of a
  few shapes with room for a name and a line, beds, hedges, trees, gates,
  benches, the fountain, path pieces and the field's edge. Every model's
  source and licence is written next to it.
- **Light:** the moon is the only key light, dim and blue, with soft shadows
  and a light outline on shapes and paths so they read in the dark.
- **Camera:** over the shoulder, orbiting freely, with zoom, on keyboard,
  mouse and touch. It doesn't pass through tombstones or hedges.
- **You:** a soft-edged shadow with a light trail of particles while you move.
- **Telling us about your pet:** write about it and add photos or a short
  video. Uploads are private to you, limited in size and type, and you can
  delete them. They are used only for your pet: its avatar and its memorial
  page. The upload screen says so.
- **The avatar:** an agent reads what you shared and sets the dog's breed and
  shape, size, proportions, coat colours and pattern, ears, tail and the
  details that made it yours. Each choice is shown, and you can change any of
  it by hand in the editor, with sliders and swatches, watching the pet turn
  and move.
- **Emotes:** six to start: sit, play bow, roll over, howl, wag and sleep. The
  agent makes new ones from a sentence ("she always spun twice before lying
  down").
- **Skills:** chosen when you make the pet and saved with it: fetch, obedience
  (sit, stay, come) and digging holes. Each one has an action in the field.
- **Tombstones:** when you press play, your pet's tombstone takes a plot for
  every visitor to see: its name, dates and a line, its achievements if it was
  competitive, and its most loved traits and stories. Walking up to one shows
  its name, its line and a link to its memorial.
- **Memorial pages:** one template for every pet: its avatar, what you shared
  when you made it, then stories, achievements and loved traits, then comments
  from anyone. The owner edits the page and can remove comments. Each memorial
  has a short link with a share card (the pet in moonlight) that opens for
  someone who isn't logged in.
- **Play:** throw a ball in the field and a dog with fetch runs for it and
  brings it back. Your pet follows you through the garden, and you can visit
  any tombstone and open its memorial.
- **Together:** you see other visitors' shadows and their pets move in real
  time. A room holds a bounded number of visitors. The chat box sits in a
  corner like Horse Isle's, with each visitor's name and the pet they brought;
  messages are limited in length and moderated.
- **Flowers:** three kinds grow and glow in the field. Picked ones go in your
  basket and grow back after a while. The arrangement you make is saved with
  the vase, and a placed vase stays for everyone.
- **The first visit:** a tutorial, one tip at a time, takes you from landing
  to making your pet, playing fetch, picking flowers and placing the vase by
  the tombstone.
- **Analytics:** each step is recorded on the openagents.com analytics: the
  galaxy choice, the flight, the pet made, emotes, fetch, flowers, the vase
  and memorial shares.
- **Safety:** uploads, memorial text, comments and chat can be reported and
  removed by their owner, and reports go to a review queue. Grief is
  personal: nothing a visitor shares is used for anything but their pet.
- **Launch:** Kiki plays the whole expansion and signs off before it opens to
  everyone.

## How it's built

The roadmap is on the project page as tasks, grouped into milestones:

- the arrival;
- the moonlit world;
- you, the visitor;
- bringing your pet;
- tombstones and memorials;
- play;
- others in the garden;
- the flowers;
- launch.

Some mechanics need upgrades to the Verse engine. Each of those is a task in
the Verse project (https://openagents.com/projects/verse, under "What Planetary
Villa needs from the engine"). The two tasks link to each other, and the
Planetary Villa task waits on the Verse one.

## Open questions for Kiki

Kiki answered the first round on October 28; the answers are in the sections
above. Still open:

- **Clouds:** are there clouds in the sky?
- **Cleaning up:** who can clean up a placed vase: whoever placed it, the pet's
  owner, or anyone?
