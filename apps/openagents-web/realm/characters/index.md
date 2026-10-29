# Characters

A Verse realm at https://openagents.com/realm/characters that shows the engine's characters: figures built from a few dozen rounded bones, posed and moved by the engine and drawn in a night garden.

- Two shadow visitors: soft silhouettes, black all through, their edges a fuzzy fringe that thins into the night, lit by nothing but a rim of moonlight.
- A guest drawn lit, in its coat's colours, to compare.
- Two four-legged animals: a dog in socks, and a small fox with big ears, a long thick tail and dark points, trotting a ring.

Choose a figure by tapping or clicking it, then its gait (STAND, WALK, RUN, and TROT for an animal), its look (SOFT, a silhouette, or LIT) and an emote, which plays over its gait: a person can WAVE, BOW, CHEER, SHRUG or DANCE (and wave while it walks); an animal can SIT, LIE, DIG, FETCH (pick something up and lift its head), BOW (a play bow, tail wagging) or WAG. Sitting, lying and digging stop the animal; a gait ends them. SHAPE opens a panel with a slider for each number of the figure's build, its coat's pattern and its colours. Drag to turn the view; the wheel zooms. On a computer, 1, 2, 3 and T pick the gait, L the look, S the shape panel, 4 to 9 the emotes, and the arrow keys or Tab the figure.

## Shapes an agent can write

A figure's build is a short document too. Put it in the link and the chosen figure takes it, or the page says which line it refused and why:

    https://openagents.com/realm/characters#shape=size 0.7; legs 0.85; snout 1.35; ear_size 1.5; tail 1.6; fluff 1.9; pattern points; coat 0.66 0.24 0.06; marks 0.03 0.03 0.03

Each statement is a name and its value; what isn't named keeps its value. The numbers and their ranges (1 is the usual): `size` (0.4..1.8, the whole figure), `legs` (0.5..1.6), `body` (0.6..1.6, its length), `girth` (0.6..1.6), `head` (0.7..1.5), and for an animal `snout` (0.4..1.6), `ears` (0 drops them, 1 stands them up), `ear_size` (0.5..1.8), `tail` (0..1.8, 0 for none) and `fluff` (0.5..2, the tail's thickness). `pattern` is `plain`, `patches`, `points` (dark legs, ears, muzzle and tail tip), `stripes` or `socks`; `coat R G B` and `marks R G B` are colours, each 0..1; `look` is `soft` or `lit`.

## Motions an agent can write

An emote is a small text document, a statement a line (or separated by `;`, so a whole one fits in a link). The engine checks it, plays it and blends it over the figure's gait. Put one in the link and the page plays it on a figure of its rig (YOURS among the emotes), or says on the screen which line it refused and why:

    https://openagents.com/realm/characters#motion=nod; rig person; seconds 1.2; loop; at 0; head 20 0 0; at 0.6; head -15 0 0

The statements:

- `motion NAME`: its name, a-z, 0-9, `_` and `-`, at most 32 long. In a link the space after `motion` may be `=`.
- `rig person` or `rig animal`.
- `seconds N`: how long it lasts, 0.1 to 30.
- `once` (the default: it fades out at its end), `loop` (it starts over until it is stopped; the same emote again stops it) or `hold` (it keeps its last pose).
- `fade N`: seconds it takes to blend in and out, 0 to 2 (0.25 if left out).
- `at T`: a key, T seconds from the start, each later than the last.
- `JOINT PITCH YAW ROLL`: under a key, where a joint turns to, in degrees.
- `lift X Y Z`: under a key, the whole figure moved, in metres (up to 1.5 across and along, -1 to 1.5 up).
- `#` starts a note to the end of the line.

Between two keys a joint eases from one turn to the next; a joint moves only in the keys that name it, and the rest of the body keeps doing what it was doing. A loop eases from its last key back round to its first.

Angles: a figure faces forward along its own z, its left along x and up along y. Pitch turns about the left axis: positive tips a bone that points up forward (a bow, a nod) and swings one that hangs down backward (an arm, a leg); so an arm raised in front is a negative pitch. Yaw turns about the up axis, positive toward the figure's left. Roll turns about the forward axis: positive tips toward the figure's left, so `left_shoulder 0 0 90` holds the left arm straight out and `right_shoulder 0 0 -170` raises the right arm overhead. Every turn is checked against its joint's limits; a turn outside them is refused (the message names the joint, the axis and the range), never bent to fit.

A person's joints and their limits (pitch, yaw, roll, in degrees): `hips` (free: it turns the whole figure), `spine` and `chest` (-35..70, -45..45, -35..35), `neck` (-45..60, -75..75, -35..35), `head` (-45..45, -60..60, -35..35), `left_shoulder` (-190..70, -100..100, -40..190), `right_shoulder` (-190..70, -100..100, -190..40), `left_elbow` and `right_elbow` (-155..5, -100..100, -10..10), `left_hand` and `right_hand` (-80..80, -40..40, -60..60), `left_hip` (-125..50, -50..50, -30..80), `right_hip` (-125..50, -50..50, -80..30), `left_knee` and `right_knee` (-5..155, -10..10, -10..10), `left_foot` and `right_foot` (-50..50, -30..30, -25..25); `crown`, `left_toe` and `right_toe` are ends and don't turn.

An animal's joints: `pelvis` (free), `spine` and `chest` (-40..40, -40..40, -30..30), `neck` (-70..80, -80..80, -40..40), `head` (-60..60, -60..60, -45..45), `left_ear` and `right_ear` (-70..70, -60..60, -70..70), `tail`, `tail_2` and `tail_3` (-80..100, -80..80, -40..40); each leg, `front_left_`, `front_right_`, `hind_left_` and `hind_right_` followed by its joints from the top: a front leg's `shoulder` (-130..80, -25..25, -40..40), `elbow` (-10..150, -10..10, -10..10), `wrist` (-40..130, -10..10, -10..10) and `paw` (-80..80, -20..20, -20..20); a hind leg's `hip` (-110..80, -25..25, -40..40), `knee` (-30..130, -10..10, -10..10), `hock` (-130..40, -10..10, -10..10) and `paw` (-80..80, -20..20, -20..20). `nose`, the ear tips, `tail_tip`, the toes and the eyes are ends and don't turn.

The engine's own emotes are written in this same form (`verse/motion.coax`).

## How it is made

- `verse/figure.coax` is the engine's figure: a rig (a person, or a four-legged animal), a shape (size, legs, body, girth, head, and for an animal its snout, ears, tail and coat) and a pose (each joint's turn, held to the joint's limits). It places the posed bones in the world and works out the gaits from a clock.
- An animal's gaits: a walk lifts one foot at a time (hind left, front left, hind right, front right), a trot moves diagonal pairs together, and a run is a bounding gallop with the spine flexing.
- `verse/motion.coax` reads and checks motion documents and plays and blends them over a pose.
- `apps/openagents-web/characters.coax` is the realm: the figures, where they walk, the view and the controls.
- `apps/openagents-web/characters.wgsl` draws it: each pixel's ray is marched through the figures' bones, melted into one smooth body; a soft silhouette's edge is how near the ray passed, so it fades out instead of ending at a line.

Without WebGPU (a current Chrome, Edge or Safari) the page shows only its name and a link here.
