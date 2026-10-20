// Starts AgentGraph: loads the wasm-bindgen glue beside this file, then hands
// the module the graph the page carries (`#agent-graph-spec`), or the
// `graph.json` beside this file. The canvas goes in `#gpui-mount`, over the
// page's own map, which stays when the module can't start.
import init, { mount } from "./openagents_web_agent_graph.js";

var box = document.getElementById("gpui-mount");

function graph() {
  var spec = document.getElementById("agent-graph-spec");
  if (spec) {
    return Promise.resolve(spec.textContent);
  }
  return fetch(new URL("./graph.json", import.meta.url)).then(function (r) {
    return r.text();
  });
}

if (box) {
  var still = window.matchMedia("(prefers-reduced-motion: reduce)").matches;
  Promise.all([init(), graph()])
    .then(function (ready) {
      box.classList.add("is-live");
      mount(ready[1], still);
    })
    .catch(function (error) {
      box.classList.remove("is-live");
      box.querySelectorAll("canvas").forEach(function (canvas) {
        canvas.remove();
      });
      console.error(error);
    });
}
