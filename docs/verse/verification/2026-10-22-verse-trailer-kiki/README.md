# The Verse trailer, second cut: Kiki's storyboard (#12458)

[`contact.jpg`](contact.jpg) shows one frame from each of the cut's 17 shots,
in order. The cut is [`verse-trailer-kiki.json`](../../film/verse-trailer-kiki.json),
72 s at 1920×1080 and 30 fps, in Video Lab as `verse-trailer-kiki`. The
review render is `~/work/film-out/cut/verse-trailer-kiki.mp4` on the owner's
Mac, with sheets under `~/work/film-out/cut/verse-trailer-kiki-sheets/`.

`scripts/film/cut.py check` and `inspect` both report `17 shots, 72.0s, 0
problem(s)`.

| Beat (her timing) | Voiceover | Shots | Real or vision |
| --- | --- | --- | --- |
| 1 Enter the Verse (0–8 s) | "What if anyone could create a world of their own?" | The Grid to the horizon with people and agents walking; agents as ghosts, and a portal's arch rising and opening | Grid walk real; ghosts and portal VISION |
| 2 Create with your voice (8–20 s) | "Imagine it. Describe it. Build it with AI—from wherever you are." | A phone: "Make me a haunted house, with a werewolf that howls at you," the agent's steps; then the house rising out of the Grid in that order, the werewolf a ghost | VISION |
| 3 Step into something new (20–32 s) | "From a shared universe to worlds limited only by your imagination." | The world spreads from the door over the Grid at the same camera, the werewolf is summoned and howls, and the eye walks up the path; the Camping Adventure's mountains toggled from Grid to world; the Everglade | VISION, except the Everglade |
| 4 More than a world (32–45 s) | "Create games. Build experiences. Bring your ideas to life for others to explore." | The Ice Cream Man played; Blue Rush Studios' main street (theater, shop, museum, legal hall, power plant); the shopkeeper's digital twin talking with a visitor and her dog; the ritual chamber | Ice Cream Man and ritual real; street and shop VISION |
| 5 Make it yours (45–58 s) | "Share what you create. Find your audience. Earn from your worlds." | Home plots: hers publishes, lights gold, and becomes its world; ghosts cross the Grid and turn solid inside it; visitors round the howling werewolf, gold motes rising | VISION |
| 6 The reveal (58–72 s) | "The Verse. Your imagination is the starting point." | Pull back from her plot on the OpenAgents planet, the Grid wrapped round a sphere, to a galaxy: Blue Rush's planet, an Earth twin, brand planets, threads of light; THE VERSE wordmark | VISION; the wordmark stands in for a logo |

The vision shots are `film_shot`'s `astral` scene; the Haunted House is a
stand-in until its zone (#12477) lands. The voice is Bella, one recording
per line, placed with `lab_voice`. The music is `music.py --mood warm`.
