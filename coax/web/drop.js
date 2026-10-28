const STATE_KIND = 40, VIEWER_KIND = 3;
const ACT_PICK = 20, ACT_SIGNIN = 22, ACT_DRAFT = 23;
const PARALLEL = 3;

export function startDrops({ send }) {
  const files = [];
  let signed = "", dragging = false, dropId = "", making = null, depth = 0, note = "", draft = null, resumed = false;
  const picker = Object.assign(document.createElement("input"), { type: "file", multiple: true });
  picker.hidden = true;
  document.body.appendChild(picker);
  const clean = (s) => String(s ?? "").replace(/[|\r\n\t]/g, " ");
  const signer = () => !!(globalThis.nostr && typeof globalThis.nostr.signEvent === "function");

  function push() {
    const lines = [`S|${signed ? 1 : 0}|${signer() ? 1 : 0}|${dragging ? 1 : 0}|${clean(note)}`];
    for (const f of files.slice(-40)) lines.push(`F|${clean(f.name).slice(0, 80)}|${f.kind}|${f.bytes}|${f.permille}|${f.status}|${clean(f.note).slice(0, 60)}`);
    if (draft) lines.push(draftLine(draft));
    send(STATE_KIND, lines.join("\n"));
  }

  function draftLine(x) {
    const d = x.draft || {}, ms = d.roadmap?.milestones || [];
    const tasks = ms.reduce((n, m) => n + (m.tasks?.length || 0), 0);
    return `D|${x.status}|${clean(d.card?.name).slice(0, 80)}|${clean(d.card?.line).slice(0, 200)}|${ms.length}|${tasks}|${d.mechanics?.length || 0}|${d.design?.open_questions?.length || 0}|${d.engine_needs?.length || 0}|${clean(x.note).slice(0, 120)}`;
  }

  async function get(url) {
    const r = await fetch(url, { credentials: "same-origin", cache: "no-store" });
    return r.ok ? r.json() : null;
  }

  async function resume() {
    if (resumed || dropId) return;
    resumed = true;
    try {
      const list = await get("/api/drops");
      const open = list?.drops?.find((d) => d.status === "open" && d.files > 0);
      if (!open || dropId) return;
      dropId = open.id;
      const d = await get(`/api/drops/${open.id}`);
      for (const f of d?.files || []) if (f.status !== "uploading" && f.status !== "waiting")
        files.push({ file: null, name: f.name, kind: f.kind, bytes: f.bytes, permille: 1000, status: f.status === "scanned" ? "kept" : "refused", note: f.note });
      const x = await get(`/api/drops/${open.id}/draft`);
      if (x && x.status !== "none") draft = x;
    } catch {}
    push();
  }

  async function read() {
    if (!dropId || draft?.status === "reading") return;
    draft = { status: "reading", note: "" };
    push();
    try {
      const [, x] = await post(`/api/drops/${dropId}/draft`, {});
      draft = x && x.status ? x : { status: "failed", note: x?.error || "no answer" };
    } catch { draft = { status: "failed", note: "no answer" }; }
    push();
  }

  async function me() {
    try {
      const r = await fetch("/api/drops/me", { credentials: "same-origin", cache: "no-store" });
      const j = r.ok ? await r.json() : null;
      signed = j && j.may ? j.pubkey : "";
      note = j && !j.may ? "this key may not drop files yet" : "";
    } catch { signed = ""; }
    if (signed) send(VIEWER_KIND, signed);
    push();
    if (signed) { pump(); resume(); }
  }

  async function signin() {
    if (!signer()) { note = "a Nostr signer is needed"; push(); return; }
    const url = `${location.origin}/api/drops/signin`;
    try {
      const ev = await globalThis.nostr.signEvent({ kind: 27235, created_at: Math.floor(Date.now() / 1000), content: "", tags: [["u", url], ["method", "POST"]] });
      const auth = btoa(String.fromCharCode(...new TextEncoder().encode(JSON.stringify(ev))));
      const r = await fetch("/api/drops/signin", { method: "POST", credentials: "same-origin", headers: { Authorization: `Nostr ${auth}` } });
      note = r.ok ? "" : ((await r.json().catch(() => ({}))).error || "sign-in refused");
    } catch { note = "sign-in was cancelled"; }
    push();
    await me();
  }

  function take(list) {
    for (const file of list) files.push({ file, name: file.name, kind: "", bytes: file.size, permille: 0, status: "queued", note: "" });
    push();
    if (signed) pump(); else me();
  }

  async function post(url, body) {
    const r = await fetch(url, { method: "POST", credentials: "same-origin", headers: { "Content-Type": "application/json" }, body: JSON.stringify(body || {}) });
    const j = await r.json().catch(() => ({}));
    return [r.status, j];
  }

  function ensureDrop() {
    if (dropId) return Promise.resolve(dropId);
    making ||= post("/api/drops", {}).then(([s, j]) => { making = null; if (s !== 200) throw new Error(j.error || "no drop"); return (dropId = j.id); });
    return making;
  }

  function put(f, url) {
    return new Promise((ok) => {
      const x = new XMLHttpRequest();
      x.open("PUT", url);
      x.upload.onprogress = (e) => { if (e.lengthComputable) { f.permille = Math.min(999, Math.floor((e.loaded / e.total) * 1000)); push(); } };
      x.onload = () => ok(x.status >= 200 && x.status < 300);
      x.onerror = x.onabort = () => ok(false);
      x.send(f.file.slice(0, f.file.size, ""));
    });
  }

  async function upload(f) {
    try {
      const d = await ensureDrop();
      const [s, j] = await post(`/api/drops/${d}/files`, { name: f.name, bytes: f.bytes });
      if (s !== 200) { f.status = "refused"; f.note = j.error || "refused"; return; }
      f.id = j.file.id; f.kind = j.file.kind; f.status = "uploading"; push();
      if (!(await put(f, j.upload))) { f.status = "refused"; f.note = "the upload failed; drop it again"; return; }
      f.permille = 1000; f.status = "scanning"; push();
      const [s2, k] = await post(`/api/drops/${d}/files/${f.id}/done`, {});
      if (s2 !== 200) { f.status = "refused"; f.note = k.error || "not scanned"; return; }
      f.status = k.file.status === "scanned" ? "kept" : "refused"; f.note = k.file.note || "";
    } catch (e) { f.status = "refused"; f.note = e.message || "failed"; }
    finally { f.file = f.status === "kept" || f.status === "refused" ? null : f.file; push(); pump(); }
  }

  function pump() {
    if (!signed) return;
    let busy = files.filter((f) => f.status === "uploading" || f.status === "scanning" || f.status === "starting").length;
    for (const f of files) {
      if (busy >= PARALLEL) break;
      if (f.status !== "queued") continue;
      f.status = "starting"; busy++;
      upload(f);
    }
  }

  const hasFiles = (e) => [...(e.dataTransfer?.types || [])].includes("Files");
  addEventListener("dragenter", (e) => { if (!hasFiles(e)) return; e.preventDefault(); depth++; if (!dragging) { dragging = true; push(); } });
  addEventListener("dragover", (e) => { if (!hasFiles(e)) return; e.preventDefault(); e.dataTransfer.dropEffect = "copy"; });
  addEventListener("dragleave", (e) => { if (!hasFiles(e)) return; depth = Math.max(0, depth - 1); if (!depth && dragging) { dragging = false; push(); } });
  addEventListener("drop", (e) => { if (!hasFiles(e)) return; e.preventDefault(); depth = 0; dragging = false; take([...e.dataTransfer.files]); });
  picker.addEventListener("change", () => { take([...picker.files]); picker.value = ""; });
  addEventListener("coaxial", (e) => {
    const code = e.detail?.code;
    if (code === 0) { push(); if (signed) send(VIEWER_KIND, signed); }
    else if (code === ACT_PICK) picker.click();
    else if (code === ACT_SIGNIN) signin();
    else if (code === ACT_DRAFT) read();
  });
  setTimeout(push, 1500);
  me();
}
