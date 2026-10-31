// Firm reason: 1 (browser host: the glue between the page's DOM, WebGPU, audio or workers and Coax's wasm)
// A design document dropped on a realm (docs/verse/realm-edit.md, "From a document"): the app asks
// with action 31, its text this site's URL ("/api/...") and, after a line break, a JSON object of
// fields to send with it (the builder's setup). The browser's file picker opens for .md, .txt, .pdf
// and .docx; the file picked is sent whole, {name, data (base64), ...fields}, with the playtest
// player key, and the server takes its text out. The app hears message 54: "1\nNAME" as it goes,
// then "STATUS\nBODY" (the server's answer). Nothing is kept here.
const ACT_DOC = 31, DOC_KIND = 54, MOST = 360 * 1024;
const ACCEPT = ".md,.markdown,.txt,.pdf,.docx,text/markdown,text/plain,application/pdf,application/vnd.openxmlformats-officedocument.wordprocessingml.document";

export function startDocs({ send }) {
  let target = "", extra = {}, picker = null;
  const key = () => { try { return JSON.parse(localStorage.getItem("verse.player") || "null")?.key || ""; } catch { return ""; } };
  const base64 = (buf) => {
    const b = new Uint8Array(buf); let s = "";
    for (let i = 0; i < b.length; i += 0x8000) s += String.fromCharCode.apply(null, b.subarray(i, i + 0x8000));
    return btoa(s);
  };
  async function one(file) {
    if (file.size > MOST) { send(DOC_KIND, "413\ntoo large"); return; }
    send(DOC_KIND, `1\n${String(file.name).replace(/[\r\n]/g, " ").slice(0, 80)}`);
    try {
      const data = base64(await file.arrayBuffer());
      const r = await fetch(target, { method: "POST", credentials: "same-origin", cache: "no-store",
        headers: { "Content-Type": "application/json", "X-Verse-Player": key() }, body: JSON.stringify({ ...extra, name: file.name, data }) });
      send(DOC_KIND, `${r.status}\n${await r.text()}`);
    } catch { send(DOC_KIND, "0\n"); }
  }
  addEventListener("coaxial", (e) => {
    const { code, text } = e.detail || {};
    if (code !== ACT_DOC) return;
    const cut = String(text || "").indexOf("\n");
    const url = cut < 0 ? text : text.slice(0, cut);
    if (!/^\/api\/[A-Za-z0-9/_-]+(\?as=lines)?$/.test(url || "")) return;
    target = url; extra = {};
    if (cut >= 0) { try { const o = JSON.parse(text.slice(cut + 1)); if (o && typeof o === "object" && !Array.isArray(o)) extra = o; } catch {} }
    if (!picker) {
      picker = Object.assign(document.createElement("input"), { type: "file", accept: ACCEPT });
      picker.hidden = true;
      document.body.appendChild(picker);
      picker.addEventListener("change", () => { const f = picker.files?.[0]; picker.value = ""; if (f) one(f); });
    }
    picker.click();
  });
  // a document dragged onto the page goes where the last one was asked for
  addEventListener("dragover", (e) => { if (target && e.dataTransfer?.types?.includes("Files")) e.preventDefault(); });
  addEventListener("drop", (e) => {
    if (!target || !e.dataTransfer?.files?.length) return;
    e.preventDefault();
    one(e.dataTransfer.files[0]);
  });
}
