import { runAcpLiveProbe, type LiveProfile } from "../src/live.ts";

const peer = process.argv[2];
if (peer !== "grok" && peer !== "cursor") throw new TypeError("usage: live-probe.ts grok|cursor");
const result = await runAcpLiveProbe(peer as LiveProfile);
process.stdout.write(`${JSON.stringify(result, null, 2)}\n`);
if (result.result === "fail") process.exitCode = 1;
