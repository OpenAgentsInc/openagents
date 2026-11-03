# MetaHuman Lab

A Verse world at https://openagents.com/worlds/metahuman-lab, not listed on the worlds page. It's the test bench for the human runtime in Aethyr, the engine every Verse world runs on. One human figure stands on a lit studio floor. A skeleton drives it, and you can play twelve animation clips on it and look at it from any side.

## Controls

- Drag to orbit the camera round the figure. The arrow keys orbit it too.
- Use the mouse wheel, a pinch, or the + and - keys to zoom.
- Shift-drag or drag with two fingers to pan.
- Hold W, A, S or D to fly the camera forward, left, back or right. Hold Q to go down and E to go up. The camera speeds up and slows down smoothly.
- Press K, or select SKELETON, to show the skeleton over the body. Each bone is drawn from its parent joint, left side in cyan and right side in orange, and the main joints show their names.
- Press N for the next clip and P for the one before, or select the clip's name. The clips are idle, walk, talk, nod, fold, lantern, spell, slash, run, die, knock and cower. Walk and run carry the figure round a circle on the floor.
- Press G, or select MALE or FEMALE, to switch between the two bodies.
- Press Space to pause the clip, and C to put the camera back.
- Select WORLDS at the top right, or press Escape, to go to the [worlds list](https://openagents.com/worlds).

## What you are looking at

- The skeleton has 65 joints with the Unreal mannequin's names: `pelvis`, `spine_01` to `spine_03`, `neck_01`, `Head`, `clavicle_l`, `upperarm_l`, `lowerarm_l`, `hand_l`, the fingers, `thigh_l`, `calf_l`, `foot_l`, `ball_l`, and the same on the right. MetaHuman bodies use the same hierarchy.
- The skeletons and clips are the Quaternius Modular Character Outfits' male and female skeletons and the Universal Animation Library's clips, all CC0.
- The body is a building block made for this world, not a MetaHuman: a rounded capsule on each bone, posed from the same joints, so it bends wherever the skeleton does. A suit covers the torso, arms and legs, with skin on the head, neck and hands.

## How it is made

- `apps/openagents-web/metahumanlab.coax` is the world: the camera, the clips and the body. `aethyr/skin.coax` plays the clips on the rig in `verse/everglade/people-rig.coax`, and `aethyr/shot.coax` makes the camera's orbit.
- `aethyr/mannequin.coax` names the joints and maps another body's joint names onto them, so a body built on the same hierarchy can use this skeleton and its clips.
- `metahumanlab-face.c` binds the world for the browser, and `metahumanlab.wgsl` (from `gpu/metahumanlab.coax`) traces the floor and the figure for each pixel and lights them.
- `docs/verse/metahuman-support.md` is the plan this world starts.
