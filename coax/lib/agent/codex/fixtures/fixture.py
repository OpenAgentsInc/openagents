#!/usr/bin/env python3
"""Turns recorded `codex app-server` sessions (lines prefixed '>> ' sent,
'<< ' received, as record.py writes them) into fixtures/session.jsonl and
fixture.coax: the server's lines a client must read, scrubbed (the scratch
folder becomes /tmp/demo, the home folder /Users/me, ids of the account
dropped), with the noise a turn never reads left out (MCP startup, hooks,
moderation, read state, warnings) and long streams cut to their first deltas.
The model list keeps only the fields a catalog reads.
    python3 fixtures/fixture.py SCRATCH_DIR full.log sup.log
"""
import json, os, re, sys

here = os.path.dirname(os.path.abspath(__file__))
scratch, logs = sys.argv[1], sys.argv[2:]
home = os.path.expanduser("~")
NOISE = ("mcpServer/", "hook/", "thread/readState", "turn/moderationMetadata", "warning", "configWarning",
         "remoteControl/", "thread/settings/updated", "account/updated", "thread/started", "serverRequest/resolved")

def scrub(s):
    s = s.replace(os.path.realpath(scratch), "/tmp/demo").replace(scratch, "/tmp/demo")
    s = s.replace(home, "/Users/me").replace(os.path.basename(home), "me")
    s = re.sub(r'"installationId":"[^"]*"', '"installationId":"x"', s)
    s = re.sub(r'"serverName":"[^"]*"', '"serverName":"mac"', s)
    return s

out = []
for log in logs:
    deltas = {}
    for raw in open(log):
        if not raw.startswith("<< "):
            continue
        line = raw[3:].rstrip("\n")
        m = json.loads(line)
        method = m.get("method", "")
        if any(method.startswith(n) for n in NOISE):
            continue
        if method == "item/agentMessage/delta":
            item = m["params"]["itemId"]
            deltas[item] = deltas.get(item, 0) + 1
            if deltas[item] > 40:
                continue
        if m.get("id") == 2 and "result" in m and "data" in m["result"]:
            keep = ("id", "model", "displayName", "description", "hidden", "isDefault")
            m["result"]["data"] = [dict({k: d[k] for k in keep if k in d},
                                        supportedReasoningEfforts=[{"reasoningEffort": e["reasoningEffort"]} for e in d.get("supportedReasoningEfforts", [])])
                                   for d in m["result"]["data"]]
            line = json.dumps(m, separators=(",", ":"), ensure_ascii=False)
        if m.get("id") == 1 and "result" in m:
            m["result"] = {"userAgent": m["result"].get("userAgent", ""), "codexHome": "/Users/me/.codex"}
            line = json.dumps(m, separators=(",", ":"), ensure_ascii=False)
        out.append(scrub(line))
    out.append("")   # a blank line separates the sessions

with open(os.path.join(here, "session.jsonl"), "w") as f:
    f.write("\n".join(out))
lines = [l for l in out]
with open(os.path.join(here, "..", "fixture.coax"), "w") as f:
    f.write("// Recorded `codex app-server` 0.163.0 sessions on a ChatGPT login, scrubbed\n")
    f.write("// (fixtures/fixture.py from fixtures/session.jsonl): CX_FIXTURE_FULL lists the\n")
    f.write("// folder and reads the README, then fixes a typo and runs a failing command\n")
    f.write("// (full access); CX_FIXTURE_SUPERVISED edits with apply_patch after an approval\n")
    f.write("// (accepted), has a command declined, and is interrupted mid-story.\n\n")
    groups = [[]]
    for l in lines:
        if l == "":
            groups.append([])
        else:
            groups[-1].append(l)
    for name, g in zip(("CX_FIXTURE_FULL", "CX_FIXTURE_SUPERVISED"), groups):
        f.write("%s :: [%d] string.[\n" % (name, len(g)))
        f.write(",\n".join("  " + json.dumps(l, ensure_ascii=False) for l in g))
        f.write("\n];\n\n")
