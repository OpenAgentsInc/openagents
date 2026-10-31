# The realm builder's eval (eval-builder.sh runs it): each prompt is planned by the real planner,
# then a judge model (a different, larger one) reads the prompt and the board and names every thing
# asked for, each shown (a feature on the board draws it as asked), listed (under ASKED FOR, NOT IN
# IT YET), wrong (something else stands in for it and it isn't listed) or missing. Coverage is the
# share shown or listed; the rule (nothing asked for is ever dropped) wants 100%. No words are
# matched here: the judge reads them.
#   API=http://127.0.0.1:PORT OA_NPC_TOKEN=... python3 -I eval_builder.py [--only N] [--out FILE]
import json
import os
import sys
import time
import urllib.request

PROMPTS = [
    "a floating island village with a giant tree and friendly owls",
    "only pastel colours, bison everywhere and tepees",
    "a haunted swamp at night with bats, dead trees and a witch who sells potions",
    "a snowy mountain cabin with wolves and a frozen lake",
    "a desert oasis with camels, palm trees and a market",
    "a cozy fishing village by the sea with boats and a lighthouse",
    "a medieval castle with knights, a dragon and a moat",
    "a cyberpunk city street with neon signs and flying cars",
    "a quiet zen garden with a koi pond, cherry blossom trees and stone lanterns",
    "a pirate cove with a shipwreck, treasure chests and parrots",
    "a farm with a red barn, horses, chickens and a windmill",
    "an enchanted forest with fairies, glowing mushrooms and a waterfall",
    "a moon base with astronauts, a rover and earth in the sky",
    "a dinosaur valley with a volcano and a t-rex",
    "a wild west town with a saloon, a sheriff and tumbleweeds",
    "jungle temple ruins with monkeys, vines and hidden treasure",
    "a 2D side-scrolling cave with crystals, spiders and lava",
    "a campsite in the pines at dusk with tents, a campfire and deer",
    "a candy land with lollipop trees, a chocolate river and gingerbread houses",
    "a meadow with a small blue pond, a few rocks and a shepherd with her sheep",
]

API = os.environ["API"]
TOKEN = os.environ["OA_NPC_TOKEN"]
JUDGE = os.environ.get("OA_EVAL_JUDGE", "gemini-2.5-pro")


def call(method, path, body=None, key=None):
    h = {"Content-Type": "application/json"}
    if key:
        h["X-Verse-Player"] = key
    req = urllib.request.Request(API + path, data=None if body is None else json.dumps(body).encode(), headers=h, method=method)
    try:
        with urllib.request.urlopen(req, timeout=60) as r:
            return r.status, r.read().decode()
    except urllib.error.HTTPError as e:
        return e.code, e.read().decode()


def board_of(lines):
    b = {"setup": {}, "features": [], "wanted": []}
    for line in lines.splitlines():
        head, _, rest = line.partition(" ")
        if head in ("name", "style", "ground", "time", "camera", "palette"):
            b["setup"][head] = rest
        elif head == "f":
            kind, on, label = (rest.split(" ", 2) + ["", ""])[:3]
            b["features"].append({"feature": kind, "on": on == "1", "label": label})
        elif head == "want":
            b["wanted"].append(rest)
    return b


JUDGE_SAYS = (
    "You check a realm builder. A visitor asked for a small 3D world in words; the builder answered a board: "
    "the world's setup (ground, time of day, art style, camera, palette), the features it will draw (each a label of "
    "exactly what is drawn; on false means suggested and unchecked, still drawn if checked), and a list of things "
    "asked for that it can't draw yet (shown to the visitor as ASKED FOR, NOT IN IT YET, kept for an agent to build). "
    "List every distinct thing the visitor asked for: each object, creature, person, building, place, landform or terrain, "
    "and each quality that changes what would be drawn (giant, floating, frozen, glowing, a colour). Mood words "
    "(friendly, cozy, quiet) are not things of their own. For each, say: shown, when a feature on the board (or the setup) "
    "draws it as they asked it (a stand-in of another kind, like crows for owls or round trees for palm trees, is not shown); "
    "listed, when it, or a line that plainly means it, is in the list of things not drawn yet; wrong, when something else "
    "stands in for it and it is not listed; missing, when neither. Name in 'by' the label or line that covers it."
)
JUDGE_SCHEMA = {
    "type": "object", "required": ["things"], "properties": {"things": {"type": "array", "items": {
        "type": "object", "required": ["thing", "verdict", "by"], "propertyOrdering": ["thing", "verdict", "by"],
        "properties": {"thing": {"type": "string"}, "verdict": {"type": "string", "enum": ["shown", "listed", "wrong", "missing"]},
                       "by": {"type": "string"}}}}}}


def judge(prompt, board):
    url = f"https://aiplatform.googleapis.com/v1/projects/openagentsgemini/locations/global/publishers/google/models/{JUDGE}:generateContent"
    req = {"systemInstruction": {"parts": [{"text": JUDGE_SAYS}]},
           "contents": [{"role": "user", "parts": [{"text": "The visitor said: " + json.dumps(prompt) + "\nThe board: " + json.dumps(board)}]}],
           "generationConfig": {"responseMimeType": "application/json", "responseSchema": JUDGE_SCHEMA, "temperature": 0}}
    for _ in range(3):
        r = urllib.request.Request(url, data=json.dumps(req).encode(), headers={"Authorization": "Bearer " + TOKEN, "Content-Type": "application/json"})
        try:
            with urllib.request.urlopen(r, timeout=120) as resp:
                out = json.loads(resp.read())
            text = "".join(p.get("text", "") for p in out["candidates"][0]["content"]["parts"] if not p.get("thought"))
            return json.loads(text)["things"]
        except Exception as e:  # a failed judge call is tried again
            err = e
            time.sleep(2)
    raise SystemExit(f"judge failed: {err}")


def main():
    args = sys.argv[1:]
    only = int(args[args.index("--only") + 1]) if "--only" in args else None
    out_file = args[args.index("--out") + 1] if "--out" in args else None
    results = []
    total = covered = shown = 0
    for i, prompt in enumerate(PROMPTS):
        if only is not None and i != only:
            continue
        _, raw = call("POST", "/api/playtest/player", {})
        key = json.loads(raw)["key"]
        st, lines = call("POST", "/api/playtest/realms/plan?as=lines", {"prompt": prompt}, key)
        if st != 200:
            print(f"{i:2d} plan failed {st}: {lines}")
            results.append({"prompt": prompt, "error": st})
            continue
        board = board_of(lines)
        things = judge(prompt, board)
        n = len(things)
        c = sum(1 for t in things if t["verdict"] in ("shown", "listed"))
        s = sum(1 for t in things if t["verdict"] == "shown")
        total += n; covered += c; shown += s
        print(f"{i:2d} {c}/{n} covered ({s} shown)  {prompt}")
        for t in things:
            if t["verdict"] not in ("shown", "listed"):
                print(f"     {t['verdict'].upper()}: {t['thing']} ({t['by']})")
        results.append({"prompt": prompt, "board": board, "things": things})
    if total:
        print(f"coverage {covered}/{total} = {100 * covered / total:.1f}% (shown {100 * shown / total:.1f}%)")
    if out_file:
        with open(out_file, "w") as f:
            json.dump({"coverage": covered / total if total else None, "shown": shown / total if total else None, "results": results}, f, indent=1)
    if total and covered < total:
        sys.exit(2)


main()
