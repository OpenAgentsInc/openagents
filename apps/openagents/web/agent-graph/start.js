// Starts AgentGraph: loads the wasm-bindgen glue beside this file, then hands
// the module the graph the page carries (`#agent-graph-spec`), or the
// `graph.json` beside this file. The canvas goes in `#gpui-mount`, over the
// page's own map, which stays when the module can't start.
//
// It also runs the graph's toolbar: full screen (the browser's API, or
// filling the window where there is none, as on iPhone), and Copy JSON,
// which copies the agent's whole definition. Download JSON is a plain link.
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

function say(text) {
  var status = box.querySelector(".oa-agent-graph-status");
  if (!status) {
    return;
  }
  status.textContent = text;
  clearTimeout(say.timer);
  say.timer = setTimeout(function () {
    status.textContent = "";
  }, 2000);
}

function nativeFullscreen() {
  return document.fullscreenElement || document.webkitFullscreenElement || null;
}

function isFull() {
  return nativeFullscreen() === box || box.classList.contains("is-filled");
}

function setFullLabel() {
  var button = box.querySelector('[data-graph-action="fullscreen"]');
  if (button) {
    button.textContent = isFull() ? "Exit full screen" : "Full screen";
  }
}

function fill(on) {
  box.classList.toggle("is-filled", on);
  document.documentElement.classList.toggle("oa-agent-graph-filled", on);
  setFullLabel();
}

function toggleFull() {
  if (nativeFullscreen()) {
    (document.exitFullscreen || document.webkitExitFullscreen).call(document);
    return;
  }
  if (box.classList.contains("is-filled")) {
    fill(false);
    return;
  }
  var request = box.requestFullscreen || box.webkitRequestFullscreen;
  if (request && (document.fullscreenEnabled || document.webkitFullscreenEnabled)) {
    Promise.resolve(request.call(box)).catch(function () {
      fill(true);
    });
  } else {
    fill(true);
  }
}

function copyText(text) {
  if (navigator.clipboard && navigator.clipboard.writeText) {
    return navigator.clipboard.writeText(text);
  }
  return new Promise(function (resolve, reject) {
    var area = document.createElement("textarea");
    area.value = text;
    area.setAttribute("readonly", "");
    area.className = "unseen";
    document.body.appendChild(area);
    area.select();
    var done = document.execCommand("copy");
    area.remove();
    if (done) {
      resolve();
    } else {
      reject(new Error("copy refused"));
    }
  });
}

function tools() {
  var full = box.querySelector('[data-graph-action="fullscreen"]');
  var copy = box.querySelector('[data-graph-action="copy"]');
  if (full) {
    full.hidden = false;
    full.addEventListener("click", toggleFull);
    document.addEventListener("fullscreenchange", setFullLabel);
    document.addEventListener("webkitfullscreenchange", setFullLabel);
    // Esc leaves the window-filling fallback; the browser handles its own.
    document.addEventListener(
      "keydown",
      function (event) {
        if (event.key === "Escape" && box.classList.contains("is-filled")) {
          fill(false);
        }
      },
      true,
    );
  }
  if (copy) {
    // Fetch the definition now, so the click can copy at once: Safari only
    // lets a page copy right after the click.
    var definition = fetch(copy.getAttribute("data-graph-definition"), {
      credentials: "same-origin",
    }).then(function (r) {
      if (!r.ok) {
        throw new Error("definition " + r.status);
      }
      return r.text();
    });
    var text = null;
    definition.then(function (t) {
      text = t;
    });
    copy.hidden = false;
    copy.addEventListener("click", function () {
      var ready = text !== null ? Promise.resolve(text) : definition;
      ready
        .then(copyText)
        .then(function () {
          say("Copied");
        })
        .catch(function (error) {
          say("Couldn't copy. Use Download JSON instead.");
          console.error(error);
        });
    });
  }
}

if (box) {
  tools();
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
