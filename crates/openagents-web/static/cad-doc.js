// A design's page (/cad/{doc}, #11653): STEP, STL, 3MF and drawing files
// made on this device by the CAD engine (crates/cad-wasm, the /cad build's
// worker) from the version's stored document. Served from /chat/cad/doc.js.
const page = document.querySelector("[data-cad-doc-page]");
const buttons = page ? Array.from(page.querySelectorAll("[data-cad-download]")) : [];
const status = page && page.querySelector("[data-cad-download-status]");

function say(text) {
  if (status) status.textContent = text;
}

if (page && buttons.length) {
  const doc = page.dataset.cadDoc;
  const version = page.dataset.cadVersion;
  const title = (document.getElementById("cad-doc-title") || {}).textContent || doc;
  let documentJson = null;
  let worker = null;
  let seq = 0;
  const waiting = new Map();

  const start = () => {
    worker = new Worker("/cad/worker.js", { type: "module" });
    worker.onmessage = (event) => {
      const m = event.data;
      if ("ready" in m) {
        if (m.ready) {
          for (const b of buttons) {
            b.disabled = false;
            for (const name of ["aria-disabled", "tabindex", "data-disabled"]) b.removeAttribute(name);
          }
          say("");
        } else {
          for (const b of buttons) b.hidden = true;
          say("This browser can't make these files. The GLB file still downloads.");
        }
        return;
      }
      const w = waiting.get(m.id);
      waiting.delete(m.id);
      if (!w) return;
      if (m.ok) w.resolve(m.value);
      else w.reject(new Error(m.error));
      if (m.broken) {
        worker.terminate();
        start();
      }
    };
  };

  const call = (name, args, format) =>
    new Promise((resolve, reject) => {
      const id = ++seq;
      waiting.set(id, { resolve, reject });
      worker.postMessage({ id, call: name, args, format });
    });

  const TYPES = {
    step: ["model/step", `${doc}-v${version}.step`],
    stl: ["model/stl", `${doc}-v${version}.stl`],
    "3mf": ["model/3mf", `${doc}-v${version}.3mf`],
    pdf: ["application/pdf", `${doc}-v${version}-drawing.pdf`],
    dxf: ["image/vnd.dxf", `${doc}-v${version}-drawing.dxf`],
  };

  const download = async (format) => {
    say("Making the file on this device…");
    try {
      if (!documentJson) {
        const response = await fetch(page.dataset.cadDocument, { credentials: "same-origin" });
        if (!response.ok) throw new Error("the design couldn't be read");
        documentJson = await response.json();
      }
      const bytes = format === "pdf" || format === "dxf"
        ? await call("drawing", { document: documentJson, views: ["front", "top", "right", "iso"],
          sheet: "A3", title, subtitle: `${doc} version ${version}` }, format)
        : await call("export", { document: documentJson }, format);
      const [type, name] = TYPES[format];
      const url = URL.createObjectURL(new Blob([bytes], { type }));
      const a = document.createElement("a");
      a.href = url;
      a.download = name;
      document.body.append(a);
      a.click();
      a.remove();
      setTimeout(() => URL.revokeObjectURL(url), 10000);
      say(`${name} is ready (${(bytes.length / 1024).toFixed(1)} KB).`);
    } catch (error) {
      say(`We couldn't make that file: ${error.message}`);
    }
  };

  for (const b of buttons) b.addEventListener("click", () => download(b.dataset.cadDownload));
  start();
}
