// The relay's check (room-relay.coax):
//   bun coax/web/room/relay-check.ts ws://127.0.0.1:8830   (deploy.sh runs it through wss://openagents.com)
// two sockets in one relay room: each hears its seat, the other's join, and the other's frames
const base = process.argv[2] ?? "ws://127.0.0.1:8830";
const code = "t" + (Date.now() % 100000);
const heard: string[][] = [[], []];
const open = (i: number) => new Promise<WebSocket>((res, rej) => { const ws = new WebSocket(`${base}/api/room/relay/check/${code}`); ws.onmessage = (e) => heard[i].push(String(e.data)); ws.onopen = () => res(ws); ws.onerror = rej; });
const a = await open(0); await Bun.sleep(300); const b = await open(1); await Bun.sleep(300);
a.send("hello from a"); b.send("x".repeat(3000)); await Bun.sleep(600);
let t0 = performance.now(); let got = 0; const n = 200;
for (let i = 0; i < n; i++) a.send("u " + i);
while (heard[1].filter((h) => h.startsWith("m 0 u ")).length < n && performance.now() - t0 < 5000) await Bun.sleep(20);
const ms = performance.now() - t0;
b.close(); await Bun.sleep(500);
const ok = heard[0][0] === "y 0 0" && heard[0][1] === "j 1" && heard[1][0] === "y 1 1" && heard[1][1] === "j 0" && heard[1][2] === "m 0 hello from a" && heard[0][2] === "m 1 " + "x".repeat(3000) && heard[1].filter((h) => h.startsWith("m 0 u ")).length === n && heard[0].includes("l 1");
console.log(JSON.stringify({ a: heard[0].slice(0, 3).map((h) => h.slice(0, 20)), b: heard[1].slice(0, 3).map((h) => h.slice(0, 20)), relayed: heard[1].filter((h) => h.startsWith("m 0 u ")).length, ms: Math.round(ms) }));
console.log(ok ? "relays" : "fails"); a.close(); process.exit(ok ? 0 : 1);
