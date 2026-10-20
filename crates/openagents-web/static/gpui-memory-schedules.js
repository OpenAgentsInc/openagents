// The HTML settings forms remain until the separate Settings canvas is ready.
const entry = document.getElementById("gpui-account-prompts-entry");
let mountedBox;
if (entry && navigator.gpu) {
  (async () => {
    if (!await navigator.gpu.requestAdapter()) return;
    const data = entry.dataset;
    const [module, fonts] = await Promise.all([
      import(data.module),
      Promise.all(JSON.parse(data.fonts).map(async url => new Uint8Array(await (await fetch(url)).arrayBuffer()))),
    ]);
    await module.default({ module_or_path: WebAssembly.compileStreaming(fetch(data.wasm)) });
    const box = document.createElement("div");
    box.id = "gpui-mount";
    mountedBox = box;
    box.style.cssText = "position:fixed;inset:0;z-index:7";
    document.body.appendChild(box);
    try { module.mount(fonts, data.page, JSON.stringify({ signedIn: true })); }
    catch (error) { box.remove(); throw error; }
    window.addEventListener("oa-settings-page", event => {
      history.replaceState(null, "", "/settings?ui=gpui&page=" + encodeURIComponent(event.detail));
    });
    window.addEventListener("oa-settings-close", () => location.assign("/?ui=gpui"));
    window.addEventListener("error", () => box.remove(), { once: true });
  })().catch(error => {
    if (mountedBox) mountedBox.remove();
    console.error("Settings couldn't start", error);
  });
}
