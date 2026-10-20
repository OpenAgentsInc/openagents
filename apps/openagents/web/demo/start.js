window.__oaVisualUrl = new URL("./visuals/start.js", import.meta.url).href;
// Starts the chat demo: loads the wasm-bindgen glue beside this file, which
// fetches the module next to it, and the shared font files the page names. GPUI then adds its canvas to the page, so
// the page's own content (the loading line) steps out of its way.
import { loadRenderer } from "./renderer.js";

// The faces the demo draws: the site's shared font files, which the page
// names (and starts downloading) with `<link data-gpui-font>` preloads.
function fonts() {
  var links = document.querySelectorAll("link[data-gpui-font]");
  if (!links.length) {
    return Promise.reject(new Error("the page names no fonts"));
  }
  return Promise.all(
    Array.prototype.map.call(links, function (link) { return link.href; }).concat([
      new URL("./NotoSansCJKjp-Regular-demo.otf", import.meta.url),
      new URL("./NotoColorEmoji-demo.ttf", import.meta.url),
    ]).map(function (url) {
      return fetch(url).then(function (response) {
        if (!response.ok) {
          throw new Error("font " + url + ": " + response.status);
        }
        return response.arrayBuffer();
      }).then(function (buffer) {
        return new Uint8Array(buffer);
      });
    }),
  );
}

Promise.resolve()
  .then(function () {
    return Promise.all([loadRenderer(new URL("./openagents_web_demo.js", import.meta.url), new URL("./openagents_web_demo_bg.wasm", import.meta.url), new URL("./openagents_web_demo_webgl.js", import.meta.url), new URL("./openagents_web_demo_webgl_bg.wasm", import.meta.url)), fonts()]);
  })
  .then(function (ready) {
    ready[0].launch(ready[1]);
    var status = document.getElementById("gpui-status");
    if (status) {
      status.remove();
    }
    var content = document.getElementById("content");
    if (content) {
      content.style.display = "none";
    }
  })
  .catch(function (error) {
    var status = document.getElementById("gpui-status");
    if (status) {
      status.textContent = "The demo couldn't start. Reload the page to try again.";
    }
    console.error(error);
  });
