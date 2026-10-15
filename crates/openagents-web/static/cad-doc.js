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
  let assetsReady = null;
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
        assetsReady = null;
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

  const pullPanel = page.querySelector("[data-cad-pull-panel]");
  const viewer = page.querySelector(".oa-cad-viewer");
  let pick = null, generation = 0, pending = null, previewing = false;
  const deltaInput = page.querySelector("[data-cad-pull-delta]");
  const pullStatus = page.querySelector("[data-cad-pull-status]");
  const saveButton = page.querySelector("[data-cad-pull-save]");
  let checkedDelta = null, saving = false;
  const pullSay = (text) => { if (pullStatus) pullStatus.textContent = text; };
  const loadAssets = () => assetsReady || (assetsReady = (async () => {
    const manifestUrl=page.dataset.cadDocument.replace('name=doc.json','name=assets.json');
    const response=await fetch(manifestUrl,{credentials:'same-origin'});
    if(response.status===404)return;
    if(!response.ok)throw new Error("The design's imported files couldn't be read.");
    for(const asset of await response.json()) {
      const url=page.dataset.cadDocument.replace('name=doc.json',`name=out/asset-${asset.sha256}.${asset.format}`);
      const file=await fetch(url,{credentials:'same-origin'});if(!file.ok)throw new Error('An imported model is missing.');
      const reference=await call('asset',{bytes:await file.arrayBuffer(),format:asset.format});
      if(reference!==`sha256:${asset.sha256}`)throw new Error('An imported model failed its SHA-256 check.');
    }
  })());
  const loadDocument = async () => {
    await loadAssets();
    if (!documentJson) {
      const response = await fetch(page.dataset.cadDocument, {credentials:"same-origin"});
      if (!response.ok) throw new Error("The design couldn't be read.");
      documentJson = await response.json();
    }
    return documentJson;
  };
  const savePull = async () => {
    if (!pick || checkedDelta === null || checkedDelta === 0 || saving) return;
    saving = true; saveButton.disabled = true; pullSay("Saving the change…");
    try {
      const form = new URLSearchParams({csrf:page.dataset.cadCsrf,project:page.dataset.cadProject,
        version,part:pick.part,face:pick.face,delta:String(checkedDelta)});
      const response = await fetch(page.dataset.cadPull, {method:"POST",credentials:"same-origin",body:form});
      if (!response.ok) throw new Error(await response.text());
      const result = await response.json(); window.location.assign(result.url);
    } catch (error) { pullSay(error.message); saving=false; saveButton.disabled=false; }
  };
  const pumpPreview = async () => {
    if (previewing || !pending || saving) return;
    previewing=true;
    const job=pending; pending=null; const selected=pick;
    try {
      const result = await call("run", {tool:"cad_drag_face",args:{document:await loadDocument(),part:selected.part,face:selected.face,delta:job.delta}});
      if (job.generation === generation) {
        if (result.error) throw new Error(result.error);
        viewer.__cadSetModel(result.view); checkedDelta=job.delta;
        const findings=page.querySelector('[data-cad-findings-panel]');if(findings)findings.hidden=job.delta!==0;
        viewer.dispatchEvent(new CustomEvent("cad-pull-handle", {detail:selected}));
        pullPanel.querySelector("[data-cad-pull-label]").textContent = `${result.edit.dimension}: ${Number(result.edit.value.toFixed(2))} mm`;
        pullSay(job.delta === 0 ? "Drag Pull to move this face, or enter a distance." : "Preview checked.");
        saveButton.disabled=job.delta===0;
        if (job.commit) await savePull();
      }
    } catch (error) {
      if (job.generation === generation) { checkedDelta=null; saveButton.disabled=true; pullSay(error.message); }
    } finally { previewing=false; pumpPreview(); }
  };
  const previewPull = (delta,commit=false) => {
    if (!pick || saving) return;
    checkedDelta=null; saveButton.disabled=true; deltaInput.value=String(delta);
    pending={delta,commit,generation:++generation}; pumpPreview();
  };
  const cancelPull = () => {
    if (!pick || saving) return;
    ++generation; pending=null; pick=null; checkedDelta=null; pullPanel.hidden=true;
    viewer.dispatchEvent(new CustomEvent("cad-pull-handle", {detail:null})); viewer.__cadSetModel(null);
    const findings=page.querySelector('[data-cad-findings-panel]');if(findings){findings.hidden=false;viewer.__cadHighlightFindings?.(JSON.parse(page.dataset.cadFindings||'[]'));}
  };
  if (pullPanel && viewer) {
    viewer.addEventListener("cad-face-pick", (event) => {
      if (saving) return;
      pick=event.detail; pullPanel.hidden=false;
      viewer.dispatchEvent(new CustomEvent("cad-pull-handle", {detail:null})); previewPull(0);
    });
    viewer.addEventListener("cad-face-drag", (event) => {
      if (event.detail.cancel) cancelPull(); else previewPull(event.detail.delta,event.detail.commit);
    });
    page.querySelector("[data-cad-pull-preview]").addEventListener("click",()=>previewPull(Number(deltaInput.value)));
    saveButton.addEventListener("click",savePull);
    deltaInput.addEventListener("input",()=>{++generation;pending=null;checkedDelta=null;saveButton.disabled=true;});
    page.querySelector("[data-cad-pull-cancel]").addEventListener("click",cancelPull);
    page.addEventListener("keydown",(event)=>{if(event.key==="Escape") cancelPull();});
  }

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
      await loadAssets();
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
