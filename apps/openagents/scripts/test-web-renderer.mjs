// Run under `openagents browser run -- node .../test-web-renderer.mjs`.
// Pass a local --url, --backend webgl|webgpu, --gpu missing|rejected,
// --capture FILE, optional --compare PNG, --fixture, --panels, --visuals, or --demo-static.
import assert from "node:assert/strict";
import fs from "node:fs/promises";
const args = process.argv.slice(2);
const option = (name, fallback) => args.includes(name) ? args[args.indexOf(name) + 1] : fallback;
const url = option("--url");
assert(url, "--url is required");
const backend = option("--backend", "webgl");
const browser = `http://127.0.0.1:${process.env.OPENAGENTS_CHROME_PORT}`;
const page = await (await fetch(`${browser}/json/new?about:blank`, { method: "PUT" })).json();
const ws = new WebSocket(page.webSocketDebuggerUrl);
await new Promise(resolve => ws.addEventListener("open", resolve, { once: true }));
let sequence = 0;
const pending = new Map(), errors = [], requests = [];
ws.addEventListener("message", event => {
  const message = JSON.parse(event.data);
  if (message.id) {
    const call = pending.get(message.id);
    pending.delete(message.id);
    message.error ? call.reject(message.error) : call.resolve(message.result);
  } else if (message.method === "Runtime.exceptionThrown") {
    errors.push(message.params.exceptionDetails);
  } else if (message.method === "Runtime.consoleAPICalled" && message.params.type === "error") {
    errors.push(message.params.args);
  } else if (message.method === "Network.requestWillBeSent") {
    requests.push(message.params.request.url);
  }
});
const send = (method, params = {}) => new Promise((resolve, reject) => {
  const id = ++sequence;
  pending.set(id, { resolve, reject });
  ws.send(JSON.stringify({ id, method, params }));
});
const evaluate = async expression => {
  const result = await send("Runtime.evaluate", { expression, awaitPromise: true, returnByValue: true });
  assert(!result.exceptionDetails, JSON.stringify(result.exceptionDetails));
  return result.result.value;
};
const comparePixels = (previous, current) => evaluate(`(async()=>{
      async function pixels(data){const image=await createImageBitmap(new Blob([Uint8Array.from(atob(data),c=>c.charCodeAt(0))],{type:"image/png"}));const c=new OffscreenCanvas(image.width,image.height);const ctx=c.getContext("2d");ctx.drawImage(image,0,0);return ctx.getImageData(0,0,c.width,c.height).data;}
      const a=await pixels(${JSON.stringify(previous)}), b=await pixels(${JSON.stringify(current)});
      if(a.length!==b.length)throw Error("different image sizes");
      let exact=0,close=0,error=0;for(let i=0;i<a.length;i+=4){let maximum=0;for(let c=0;c<3;c++){const d=Math.abs(a[i+c]-b[i+c]);maximum=Math.max(maximum,d);error+=d;}if(maximum===0)exact++;if(maximum<=3)close++;}
      return {exact:exact/(a.length/4),withinThree:close/(a.length/4),meanChannelError:error/(a.length/4*3)};
    })()`);
// SwiftShader can render WebGPU correctly while its compositor capture is
// blank. Reading the canvas forces a GPU readback of the actual frame.
const capturePixels = async () => backend === "webgpu"
  ? { data: await evaluate('document.querySelector("canvas").toDataURL("image/png").split(",")[1]') }
  : send("Page.captureScreenshot", { format: "png" });
const delay = ms => new Promise(resolve => setTimeout(resolve, ms));
const until = async expression => {
  for (let attempt = 0; attempt < 120; attempt++) {
    if (await evaluate(expression)) return;
    await delay(250);
  }
  throw Error(`Timed out: ${expression}; errors=${JSON.stringify(errors)}`);
};
try {
  await send("Page.enable");
  await send("Page.bringToFront");
  await send("Runtime.enable");
  await send("Network.enable");
  if (option("--gpu") === "rejected") await send("Network.setBlockedURLs", { urls: ["*gpui-probe*"] });
  if (option("--mbps")) await send("Network.emulateNetworkConditions", {
    offline: false, latency: Number(option("--latency-ms", "0")), downloadThroughput: Number(option("--mbps")) * 125000, uploadThroughput: -1,
  });
  if (args.includes("--render-only")) await send("Emulation.setEmulatedMedia", { features: [{ name: "prefers-reduced-motion", value: "reduce" }] });
  await send("Emulation.setDeviceMetricsOverride", { width: 1280, height: 820, deviceScaleFactor: 1, mobile: false });
  let injected = 'document.addEventListener("gpui-first-frame",()=>window.__rendererFirstFrameAt=performance.now(),{once:true});';
  if (option("--gpu") === "missing") injected += 'Object.defineProperty(navigator,"gpu",{value:undefined,configurable:true});';
  if (option("--gpu") === "rejected") injected += 'Object.defineProperty(navigator,"gpu",{value:{requestAdapter:()=>Promise.reject(new Error("adapter refused"))},configurable:true});';
  await send("Page.addScriptToEvaluateOnNewDocument", { source: injected });
  await send("Page.navigate", { url });
  await until('!!window.__rendererFirstFrameAt || document.documentElement?.dataset.gpui === "ready"');
  assert.equal(await evaluate("document.documentElement.dataset.gpuiRenderer"), backend);
  // Standalone Settings handles keys without initializing the chat exports.
  await send("Input.dispatchKeyEvent", { type: "keyDown", key: "Shift", code: "ShiftLeft" });
  await send("Input.dispatchKeyEvent", { type: "keyUp", key: "Shift", code: "ShiftLeft" });
  if (option("--max-ready-ms")) assert(await evaluate("window.__rendererFirstFrameAt") < Number(option("--max-ready-ms")), "first frame exceeds its time budget: " + await evaluate("window.__rendererFirstFrameAt"));
  if (args.includes("--visuals")) {
    await evaluate('import(window.__oaVisualUrl).then(()=>true)');
    await until('!!window.__oaVisualReady');
    const visual = {id:"renderer-test-visual",title:"Latency dashboard",path:"/Users/dev/.openagents/uploads/v_demo_dash-visual.luau",x:0,y:0,width:400,height:240,clip:[0,0,1280,820]};
    await evaluate(`window.dispatchEvent(new CustomEvent("oa-visual-slot",{detail:${JSON.stringify(JSON.stringify(visual))}}))`);
    await until('!!document.querySelector("#renderer-test-visual canvas") && document.querySelector("#renderer-test-visual canvas").style.visibility !== "hidden"');
    await delay(200);
    await evaluate('window.dispatchEvent(new CustomEvent("oa-visual-slot",{detail:JSON.stringify({id:"renderer-test-visual",remove:true})}))');
    await until('!document.getElementById("renderer-test-visual")');
  }
  if (args.includes("--demo-static")) {
    // The first demo chat streams; select its static catalog fixture.
    await send("Input.dispatchMouseEvent", {type:"mousePressed",x:120,y:195,button:"left",clickCount:1});
    await send("Input.dispatchMouseEvent", {type:"mouseReleased",x:120,y:195,button:"left",clickCount:1});
    await delay(300);
  }
  const capture = option("--capture");
  await delay(150);
  const screenshot = await capturePixels();
  if (capture) await fs.writeFile(capture, Buffer.from(screenshot.data, "base64"));
  let pixels, recovery;
  if (option("--compare")) {
    const previous = await fs.readFile(option("--compare"));
    pixels = await comparePixels(previous.toString("base64"), screenshot.data);
    assert(pixels.withinThree > Number(option("--min-pixel-match", "0.99")), JSON.stringify(pixels));
  }
  if (args.includes("--fixture")) {
    await evaluate('fixture.draw(document.querySelector("canvas"),1)');
    await delay(100);
    const updated = await capturePixels();
    assert(updated.data !== screenshot.data, "an atlas replacement must change the image");
  } else if (!args.includes("--render-only")) {
    for (const key of "Renderer") {
      await send("Input.dispatchKeyEvent", { type: "keyDown", key, text: key, code: "Key" + key.toUpperCase() });
      await send("Input.dispatchKeyEvent", { type: "keyUp", key, code: "Key" + key.toUpperCase() });
    }
    await delay(100);
    const typed = await capturePixels();
    assert(typed.data !== screenshot.data, "typing must draw a new frame");
  }
  if (args.includes("--panels")) {
    for (const [name, detail, box] of [["oa-settings-open",'{"page":"general","shared":{}}',"gpui-settings"],["oa-work-toggle","","gpui-work"],...["traces", "agents", "projects", "agent-lab", "labs", "cad", "claude", "data"].map(page => ["oa-page-open", page, "gpui-page-" + page])]) {
      await evaluate(`window.dispatchEvent(new CustomEvent(${JSON.stringify(name)},{detail:${JSON.stringify(detail)}}))`);
      await until(`!!document.querySelector("#${box} canvas") && document.querySelector("#${box} canvas").style.visibility !== "hidden"`);
      await evaluate(`document.getElementById(${JSON.stringify(box)}).hidden=true`);
    }
  }
  await send("Emulation.setDeviceMetricsOverride", { width: 900, height: 700, deviceScaleFactor: 2, mobile: false });
  if (args.includes("--fixture")) await evaluate('document.querySelector("canvas").width=1800; document.querySelector("canvas").height=1400; fixture.draw(document.querySelector("canvas"),1)');
  await until('document.querySelector("canvas").width === 1800');
  if (backend === "webgl") {
    await delay(150);
    const beforeRecovery = await capturePixels();
    await evaluate('window.__lostContext=document.querySelector("canvas").getContext("webgl2").getExtension("WEBGL_lose_context"); __lostContext.loseContext()');
    await delay(150);
    await evaluate('__lostContext.restoreContext()');
    await delay(300);
    if (args.includes("--fixture")) await evaluate('fixture.draw(document.querySelector("canvas"),1)');
    await until('!document.querySelector("canvas").getContext("webgl2").isContextLost()');
    const afterRecovery = await capturePixels();
    if (capture) {
      await fs.writeFile(capture + ".before-recovery.png", Buffer.from(beforeRecovery.data, "base64"));
      await fs.writeFile(capture + ".after-recovery.png", Buffer.from(afterRecovery.data, "base64"));
    }
    recovery = await comparePixels(beforeRecovery.data, afterRecovery.data);
    assert(recovery.withinThree > 0.98, "context restoration must repaint the frame: " + JSON.stringify(recovery));
  }
  assert.deepEqual(errors, [], JSON.stringify(errors));
  const modules = requests.filter(address => address.includes("openagents_web_") && address.endsWith(".wasm"));
  assert(modules.every(address => address.includes("_webgl") === (backend === "webgl")), JSON.stringify(modules));
  console.log(JSON.stringify({ backend, firstFrameMs: await evaluate("window.__gpuiReadyMs || window.__rendererFirstFrameAt"), navigationFirstFrameMs: await evaluate("window.__rendererFirstFrameAt"), modules, pixels, recovery }, null, 2));
} finally {
  ws.close();
}
