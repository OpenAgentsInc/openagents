// Starts the chat demo: loads the wasm-bindgen glue beside this file, which
// fetches the module next to it, and the shared font files the page names. GPUI then adds its canvas to the page, so
// the page's own content (the loading line) steps out of its way.
import init, { launch } from "./openagents_web_demo.js";

// The faces the demo draws: the site's shared font files, which the page
// names (and starts downloading) with `<link data-gpui-font>` preloads.
function fonts() {
  var links = document.querySelectorAll("link[data-gpui-font]");
  if (!links.length) {
    return Promise.reject(new Error("the page names no fonts"));
  }
  return Promise.all(
    Array.prototype.map.call(links, function (link) {
      return fetch(link.href).then(function (response) {
        if (!response.ok) {
          throw new Error("font " + link.href + ": " + response.status);
        }
        return response.arrayBuffer();
      }).then(function (buffer) {
        return new Uint8Array(buffer);
      });
    }),
  );
}

Promise.all([init(), fonts()])
  .then(function (ready) {
    launch(ready[1]);
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
