// Firm reason: 1 (browser host: the glue between the page's DOM, WebGPU, audio or workers and Coax's wasm)
// Uploads to this site for a page with data-uploads (Planetary Villa's pets:
// /api/villa/pets/<id>/files; coax/web/api/villa.c). The app asks with action
// 30, its text the files URL (this site's, "/api/..."): the browser's file
// picker opens (data-uploads is what it accepts), and each file picked is
// admitted (POST URL {name, bytes} with the playtest player key), PUT to the
// upload it was given, then scanned (POST URL/<file>/done). The app hears
// message 52, a line per file "name|permille|status|note" (status sending,
// scanning, kept, refused, failed), and message 53 with each scan's answer
// (the pet, as JSON). The bytes go only to the bucket the server opened.
const ACT_PICK = 30, STATE_KIND = 52, DONE_KIND = 53;

export function startUploads({ send, accept }) {
  const files = [];
  let target = "";
  const picker = Object.assign(document.createElement("input"), { type: "file", multiple: true, accept });
  picker.hidden = true;
  document.body.appendChild(picker);
  const clean = (s) => String(s ?? "").replace(/[|\r\n\t]/g, " ");
  const push = () => send(STATE_KIND, files.slice(-12).map((f) => `${clean(f.name).slice(0, 60)}|${f.permille}|${f.status}|${clean(f.note).slice(0, 80)}`).join("\n"));
  const key = () => { try { return JSON.parse(localStorage.getItem("verse.player") || "null")?.key || ""; } catch { return ""; } };
  const post = async (url, body) => {
    const r = await fetch(url, { method: "POST", credentials: "same-origin", cache: "no-store",
      headers: { "Content-Type": "application/json", "X-Verse-Player": key() }, body: JSON.stringify(body || {}) });
    let j = {}; try { j = await r.json(); } catch {}
    return { status: r.status, body: j };
  };
  const put = (url, file, f) => new Promise((ok) => {
    const x = new XMLHttpRequest();
    x.open("PUT", url);
    x.upload.onprogress = (e) => { if (e.lengthComputable) { f.permille = Math.floor(1000 * e.loaded / e.total); push(); } };
    x.onload = () => ok(x.status >= 200 && x.status < 300);
    x.onerror = x.onabort = () => ok(false);
    x.send(file);
  });
  async function one(file, url) {
    const f = { name: file.name, permille: 0, status: "sending", note: "" };
    files.push(f); push();
    const a = await post(url, { name: file.name, bytes: file.size });
    if (a.status !== 200) { f.status = "refused"; f.note = a.body.error || "we could not take it"; push(); return; }
    if (!(await put(a.body.upload, file, f))) { f.status = "failed"; f.note = "it did not finish sending"; push(); return; }
    f.status = "scanning"; f.permille = 1000; push();
    const d = await post(`${url}/${a.body.file}/done`);
    const mine = (d.body.files || []).find((x) => x.id === a.body.file);
    f.status = mine?.status === "kept" ? "kept" : "refused"; f.note = mine?.note || (d.status === 200 ? "" : d.body.error || "");
    push();
    if (d.status === 200) send(DONE_KIND, JSON.stringify(d.body));
  }
  picker.addEventListener("change", async () => {
    const list = [...picker.files]; picker.value = "";
    for (const file of list) await one(file, target);
  });
  addEventListener("coaxial", (e) => {
    const { code, text } = e.detail || {};
    if (code !== ACT_PICK || !/^\/api\/[A-Za-z0-9/_-]+$/.test(text || "")) return;
    target = text;
    picker.click();
  });
}
