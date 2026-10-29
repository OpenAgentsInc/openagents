const STATE_KIND = 40, VIEWER_KIND = 3, REVIEW_KIND = 41, VALUE_KIND = 42;
const ACT_PICK = 20, ACT_SIGNIN = 22, ACT_DRAFT = 23, ACT_EDIT = 24, ACT_SAVE = 25, ACT_PUBLISH = 26, ACT_DISCARD = 27;
const PARALLEL = 3;
const SECTIONS = [["card", 0, "CARD"], ["mechanics", 0, "MECHANICS"], ["design", 0, "DESIGN"], ["roadmap", 1, "ROADMAP"],
  ["engine_needs", 1, "ENGINE NEEDS"], ["assets", 1, "ASSETS"], ["to_make", 1, "TO MAKE"]];
const NAMES = { name: "Name", line: "One line", owner: "Owner", slug: "Slug", cover: "Cover", art_direction: "Art direction",
  markdown: "Design document", open_questions: "Open question", initiative: "Initiative", title: "Title", body: "Body",
  size: "Size", acceptance: "Check", mechanics: "Mechanic", needs: "Needs", why: "Why", reuses: "Reuses an engine task",
  file: "File", kind: "Kind", licence: "Licence", use: "Use", milestones: "Milestone", tasks: "Task", engine_needs: "Engine need",
  assets: "Asset", to_make: "To make" };
const ORDER = ["name", "title", "slug", "line", "owner", "size", "file", "kind", "licence", "use", "why", "reuses", "cover",
  "art_direction", "markdown", "open_questions", "body", "acceptance", "mechanics", "needs", "initiative", "milestones", "tasks"];
const ONE = { open_questions: "an open question", acceptance: "a check", mechanics: "a mechanic", needs: "a need" };
const named = (k) => NAMES[k] || String(k).replace(/_/g, " ").replace(/^./, (c) => c.toUpperCase());
const ordered = (o) => Object.keys(o).sort((a, b) => ((ORDER.indexOf(a) + 1 || 99) - (ORDER.indexOf(b) + 1 || 99)) || a.localeCompare(b));

export function startDrops({ send }) {
  const files = [];
  let signed = "", dragging = false, dropId = "", making = null, depth = 0, note = "", draft = null, resumed = false;
  let fields = [], reviewNote = "", reviewState = "";
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
    if (draft?.status === "ready") pushReview();
  }

  function draftLine(x) {
    const d = x.draft || {}, ms = d.roadmap?.milestones || [];
    const tasks = ms.reduce((n, m) => n + (m.tasks?.length || 0), 0);
    return `D|${x.status}|${clean(d.card?.name).slice(0, 80)}|${clean(d.card?.line).slice(0, 200)}|${ms.length}|${tasks}|${d.mechanics?.length || 0}|${d.design?.open_questions?.length || 0}|${d.engine_needs?.length || 0}|${clean(x.note).slice(0, 120)}|${x.approved ? 1 : 0}`;
  }

  function flatten(d) {
    const out = [];
    const row = (col, kind, label, path, value) => out.push({ col, kind, label, path, value });
    const walk = (col, v, path, label) => {
      if (Array.isArray(v)) {
        const strings = v.every((x) => typeof x === "string");
        v.forEach((x, i) => {
          if (x && typeof x === "object") { row(col, "G", `${named(path[path.length - 1]).toUpperCase()} ${i + 1}  ${x.title || x.file || x.slug || ""}`, null, ""); walk(col, x, [...path, i], ""); }
          else walk(col, x, [...path, i], `${label} ${i + 1}`);
        });
        if (strings) row(col, "A", `+ add ${ONE[path[path.length - 1]] || "one"}`, [...path, v.length], "");
      } else if (v && typeof v === "object") {
        for (const k of ordered(v)) {
          if (v[k] && typeof v[k] === "object" && !Array.isArray(v[k])) row(col, "G", named(k).toUpperCase(), null, "");
          walk(col, v[k], [...path, k], named(k));
        }
      } else if (typeof v === "boolean") row(col, "B", label, path, v ? "yes" : "no");
      else if (typeof v === "string") row(col, "T", label, path, v);
    };
    for (const [key, col, title] of SECTIONS) {
      if (d[key] === undefined) continue;
      row(col, "H", title, null, "");
      walk(col, d[key], [key], named(key));
    }
    return out;
  }

  function pushReview() {
    const d = draft?.draft;
    fields = d && draft.status === "ready" ? flatten(d) : [];
    const state = reviewState || (draft?.approved ? "approved" : "ready");
    const lines = [`V|${state}|${draft?.edits || 0}|${clean(reviewNote).slice(0, 160)}|${clean(d?.card?.name).slice(0, 80)}`];
    let room = 60000;
    for (const f of fields) {
      const line = `R|${f.col}|${f.kind}|${clean(f.label).slice(0, 90)}|${clean(f.value).slice(0, 140)}${f.value.length > 140 ? "..." : ""}`;
      room -= line.length + 1;
      if (room < 0) break;
      lines.push(line);
    }
    send(REVIEW_KIND, lines.join("\n"));
  }

  function edit(i) {
    const f = fields[i];
    if (f) send(VALUE_KIND, `${i}|${f.value}`);
  }

  async function save(text) {
    const bar = text.indexOf("|"), f = fields[Number(text.slice(0, bar))];
    if (!dropId || bar < 0 || !f?.path) return;
    reviewState = "saving"; reviewNote = "";
    try {
      const [s, x] = await post(`/api/drops/${dropId}/draft/edit`, { path: f.path.map(String), value: text.slice(bar + 1) });
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
      else if (what === "publish") draft = x;
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
      const open = list?.drops?.find((d) => (d.status === "open" || d.status === "approved") && d.files > 0);
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
      note = r.ok ? "" : ((await r.json().catch(() => ({}))).error || "login refused");
    } catch { note = "login was cancelled"; }
    push();
    await me();
  }

  function take(list) {
    if (!signed) return;
    for (const file of list) files.push({ file, name: file.name, kind: "", bytes: file.size, permille: 0, status: "queued", note: "" });
    push();
    pump();
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
    else if (code === ACT_EDIT) edit(Number(e.detail.text));
    else if (code === ACT_SAVE) save(e.detail.text || "");
    else if (code === ACT_PUBLISH) finish("publish");
    else if (code === ACT_DISCARD) finish("discard");
  });
  setTimeout(push, 1500);
  me();
}
