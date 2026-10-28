// Test-only CDP reader. Real wall time lets IndexedDB finish (Chrome's virtual
// clock may expire before an asynchronous storage callback is delivered).
const [profile, url] = process.argv.slice(2);
const pause = (ms) => new Promise((resolve) => setTimeout(resolve, ms));
const deadline = Date.now() + 20000;
let port;
while (!port && Date.now() < deadline) {
  try { port = (await Bun.file(`${profile}/DevToolsActivePort`).text()).split("\n")[0]; }
  catch { await pause(30); }
}
if (!port) throw new Error("test browser did not start");
const target = await (await fetch(`http://127.0.0.1:${port}/json/new?about:blank`, { method: "PUT" })).json();
const ws = new WebSocket(target.webSocketDebuggerUrl);
await new Promise((resolve, reject) => { ws.onopen = resolve; ws.onerror = reject; });
const pending = new Map(); let id = 0;
ws.onmessage = (e) => { const m = JSON.parse(e.data); if (pending.has(m.id)) { const [ok, bad] = pending.get(m.id); pending.delete(m.id); m.error ? bad(Error(m.error.message)) : ok(m.result); } };
const call = (method, params = {}) => new Promise((ok, bad) => { pending.set(++id, [ok, bad]); ws.send(JSON.stringify({ id, method, params })); });
await call("Page.navigate", { url });
let status = "not loaded";
while (Date.now() < deadline) {
  const r = await call("Runtime.evaluate", { expression: "document.getElementById('result')?.textContent || 'not loaded'", returnByValue: true });
  status = r.result.value;
  if (/^(PASS|FAIL) /.test(status)) break;
  await pause(30);
}
ws.close();
console.log(status);
if (!status.startsWith("PASS ")) process.exit(1);
