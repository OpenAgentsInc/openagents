// Firm reason: 1 (browser host: the glue between the page's DOM, WebGPU, audio or workers and Coax's wasm)
const STATE_KIND = 40, VIEWER_KIND = 3, REVIEW_KIND = 41;
const ACT_PICK = 20, ACT_SIGNIN = 22, ACT_DRAFT = 23, ACT_SAVE = 25, ACT_PUBLISH = 26, ACT_DISCARD = 27;
const PARALLEL = 3;
// A drop on a project (/projects?add=SLUG&key=KEY#new, from its page's ADD TO PROJECT) grows it: the
// draft is proposals for it (rc-later-drops), reviewed as a new project's draft is and ACCEPTed.
// The review's rows, their names and the edit's body are the page's (coax/lib/project/draft.coax, shared
// with the apps): this file sends it the draft and posts what it asks.

function target() {
  const q = new URLSearchParams(location.search), slug = q.get("add") || "", key = q.get("key") || "";
  return /^[a-z0-9-]{1,80}$/.test(slug) && /^[0-9a-f]{64}$/.test(key) ? { slug, key, name: slug.replace(/-/g, " "), member: true } : null;
}

export function startDrops({ send }) {
  const files = [], add = target();
  const blocked = () => !!add && !add.member;   // signed in, but not one of the project's members
  let signed = "", dragging = false, dropId = "", making = null, depth = 0, note = "", draft = null, resumed = false;
  let reviewNote = "", reviewState = "";
  const picker = Object.assign(document.createElement("input"), { type: "file", multiple: true });
  picker.hidden = true;
  document.body.appendChild(picker);
  const clean = (s) => String(s ?? "").replace(/[|\r\n\t]/g, " ");
  const signer = () => !!(globalThis.nostr && typeof globalThis.nostr.signEvent === "function");

  function push() {
    const lines = [`S|${signed ? (blocked() ? 2 : 1) : 0}|${signer() ? 1 : 0}|${dragging ? 1 : 0}|${clean(note)}|${add ? clean(add.name).slice(0, 80) : ""}`];
    for (const f of files.slice(-40)) lines.push(`F|${clean(f.name).slice(0, 80)}|${f.kind}|${f.bytes}|${f.permille}|${f.status}|${clean(f.note).slice(0, 60)}`);
    if (draft) lines.push(draftLine(draft));
    send(STATE_KIND, lines.join("\n"));
    if (draft?.status === "ready") pushReview();
  }

  function draftLine(x) {
    const d = x.draft || {}, ms = d.roadmap?.milestones || [];
    const tasks = ms.reduce((n, m) => n + (m.tasks?.length || 0), 0);
    return `D|${x.status}|${clean(d.card?.name).slice(0, 80)}|${clean(d.card?.line).slice(0, 200)}|${ms.length}|${tasks}|${d.mechanics?.length || 0}|${d.design?.open_questions?.length || 0}|${d.engine_needs?.length || 0}|${clean(x.note).slice(0, 120)}|${x.approved ? 1 : 0}`;
  }

  // V|state|edits|note|name|grow|bytes, then the draft once it is ready (apps/openagents-web/projects.coax, MSG_REVIEW)
  function pushReview() {
    const d = draft?.draft;
    const json = d && draft.status === "ready" ? JSON.stringify(d) : "";
    const state = reviewState || (draft?.published ? "published" : draft?.approved ? "approved" : "ready");
    send(REVIEW_KIND, `V|${state}|${draft?.edits || 0}|${clean(reviewNote).slice(0, 160)}|${clean(d?.card?.name).slice(0, 80)}|${d?.grow ? 1 : 0}|${new TextEncoder().encode(json).length}\n${json}`);
  }

  // body: the page's {"path":[...],"value":"..."}
  async function save(body) {
    if (!dropId || !body) return;
    reviewState = "saving"; reviewNote = "";
    try {
      const [s, x] = await post(`/api/drops/${dropId}/draft/edit`, body);
      if (s === 200) draft = x; else reviewNote = `Not saved: ${x.error || "refused"}`;
    } catch { reviewNote = "Not saved: no answer"; }
    reviewState = "";
    pushReview(); push();
  }

  async function finish(what) {
    if (!dropId) return;
    reviewNote = "";
    try {
      const [s, x] = await post(`/api/drops/${dropId}/${what}`, {});
      if (s !== 200) reviewNote = x.error || "refused";
      else if (what === "publish") {
        draft = x;
        if (x.published?.added !== undefined) reviewNote = `Added to openagents.com${x.published.page}: ${x.published.added} new ${x.published.added === 1 ? "task" : "tasks"}`;
        else if (x.published) reviewNote = `Live at openagents.com${x.published.page}`;
      }
      else { dropId = ""; draft = null; files.length = 0; reviewState = "discarded"; pushReview(); reviewState = ""; push(); return; }
    } catch { reviewNote = "no answer"; }
    pushReview(); push();
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
      const open = list?.drops?.find((d) => (d.status === "open" || d.status === "approved") && d.files > 0 && (d.project || "") === (add?.slug || ""));
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
      if (signed && add) {
        const h = await fetch(`/api/drops/home/${add.slug}/${add.key}`, { credentials: "same-origin", cache: "no-store" });
        const k = await h.json().catch(() => ({}));
        if (h.ok) { add.name = k.name || add.name; add.member = !!k.member; }
        else add.member = false;
        if (!add.member) note = h.ok ? `Only ${add.name}'s members add to it.` : (k.error || "This project doesn't take drops yet.");
      }
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
      note = r.ok ? "" : ((await r.json().catch(() => ({}))).error || "login refused");
    } catch { note = "login was cancelled"; }
    push();
    await me();
  }

  function take(list) {
    if (!signed || blocked()) return;
    for (const file of list) files.push({ file, name: file.name, kind: "", bytes: file.size, permille: 0, status: "queued", note: "" });
    push();
    pump();
  }

  async function post(url, body) {
    const r = await fetch(url, { method: "POST", credentials: "same-origin", headers: { "Content-Type": "application/json" }, body: typeof body === "string" ? body : JSON.stringify(body || {}) });
    const j = await r.json().catch(() => ({}));
    return [r.status, j];
  }

  function ensureDrop() {
    if (dropId) return Promise.resolve(dropId);
    making ||= post("/api/drops", add ? { project: add.slug, pubkey: add.key } : {}).then(([s, j]) => { making = null; if (s !== 200) throw new Error(j.error || "no drop"); return (dropId = j.id); });
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
    if (!signed || blocked()) return;
    let busy = files.filter((f) => f.status === "uploading" || f.status === "scanning" || f.status === "starting").length;
    for (const f of files) {
      if (busy >= PARALLEL) break;
      if (f.status !== "queued") continue;
      f.status = "starting"; busy++;
      upload(f);
    }
  }

  const hasFiles = (e) => [...(e.dataTransfer?.types || [])].includes("Files");
  addEventListener("dragenter", (e) => { if (!hasFiles(e)) return; e.preventDefault(); if (!signed) return; depth++; if (!dragging) { dragging = true; push(); } });
  addEventListener("dragover", (e) => { if (!hasFiles(e)) return; e.preventDefault(); e.dataTransfer.dropEffect = signed ? "copy" : "none"; });
  addEventListener("dragleave", (e) => { if (!hasFiles(e)) return; depth = Math.max(0, depth - 1); if (!depth && dragging) { dragging = false; push(); } });
  addEventListener("drop", (e) => { if (!hasFiles(e)) return; e.preventDefault(); depth = 0; dragging = false; take([...e.dataTransfer.files]); });
  picker.addEventListener("change", () => { take([...picker.files]); picker.value = ""; });
  addEventListener("coaxial", (e) => {
    const code = e.detail?.code;
    if (code === 0) { push(); if (signed) send(VIEWER_KIND, signed); }
    else if (code === ACT_PICK && signed) picker.click();
    else if (code === ACT_SIGNIN) signin();
    else if (code === ACT_DRAFT) read();
    else if (code === ACT_SAVE) save(e.detail.text || "");
    else if (code === ACT_PUBLISH) finish("publish");
    else if (code === ACT_DISCARD) finish("discard");
  });
  setTimeout(push, 1500);
  me();
}
