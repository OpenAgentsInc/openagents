// The room's load test (ol-room): PLAYERS sockets join one room, each
// reports a car at 60 Hz, and each counts the states it hears. Passes when
// every player hears >= 59 states a second and all hear the same ball.
//   bun coax/web/room/load.ts ws://127.0.0.1:8830 [players=4] [seconds=10]
const base = process.argv[2] ?? "ws://127.0.0.1:8830";
const players = Number(process.argv[3] ?? 4), seconds = Number(process.argv[4] ?? 10);
const room = `load-${Date.now() % 100000}`;
type P = { ws: WebSocket; heard: number; seat: number; balls: Map<number, number[]>; tick: number; gaps: number };
const ps: P[] = [];
for (let i = 0; i < players; i++) {
  const ws = new WebSocket(`${base}/api/room/orbit-league/${room}`);
  const p: P = { ws, heard: 0, seat: -1, balls: new Map(), tick: 0, gaps: 0 };
  ws.onmessage = (e) => {
    const f = String(e.data).split(" ");
    if (f[0] !== "s") return;
    const tick = Number(f[1]);
    if (p.tick && tick !== p.tick + 1) p.gaps++;
    p.tick = tick; p.seat = Number(f[2]); p.heard++;
    p.balls.set(tick, f.slice(10, 13).map(Number));
  };
  ps.push(p);
}
// every socket open (a cold service or a fresh edge can take a few seconds)
for (let i = 0; i < 50 && ps.some((p) => p.ws.readyState === WebSocket.CONNECTING); i++) await new Promise((r) => setTimeout(r, 100));
const open = ps.filter((p) => p.ws.readyState === WebSocket.OPEN).length;
let t = 0;
const send = setInterval(() => {
  t++;
  ps.forEach((p, i) => {
    if (p.ws.readyState !== WebSocket.OPEN) return;
    const z = -20000 + ((t * 300) % 40000);
    p.ws.send(`c ${i * 3000} 900 ${z} 0 0 0 10000 0 0 18000 0 0 0`);
  });
}, 1000 / 60);
const start = ps.map((p) => p.heard);
const t0 = performance.now();
await new Promise((r) => setTimeout(r, seconds * 1000));
const dt = (performance.now() - t0) / 1000;
clearInterval(send);
const rates = ps.map((p, i) => (p.heard - start[i]) / dt);
const seats = ps.map((p) => p.seat);
// the same ball at the same tick: even seats as sent, odd seats turned round (x and z negated)
const tick = Math.min(...ps.map((p) => p.tick)) - 1;
const balls = ps.map((p) => { const b = p.balls.get(tick) ?? [NaN, NaN, NaN]; return p.seat % 2 ? [-b[0], b[1], -b[2]] : b; });
const spread = Math.max(...balls.map((b) => Math.hypot(b[0] - balls[0][0], b[1] - balls[0][1], b[2] - balls[0][2])));
console.log(JSON.stringify({ room, open, seats, rates: rates.map((r) => +r.toFixed(1)), gaps: ps.map((p) => p.gaps), ball_spread_mm: spread }));
ps.forEach((p) => p.ws.close());
const ok = open === players && rates.every((r) => r >= 59) && spread < 2;
console.log(ok ? "holds" : "fails");
process.exit(ok ? 0 : 1);
