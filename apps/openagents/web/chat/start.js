// Starts the GPUI web chat (crates/openagents-web/src/pages/gpui_chat.rs).
// It needs WebGPU; without it, the page goes to the regular
// chat. Otherwise the module and the fonts download together,
// and the canvas replaces the page's outline once the app has started.
import init, { launch } from "./openagents_web_chat.js";

const FONTS = [
  "Geist.ttf",
  "Geist-Italic.ttf",
  "Geist-Medium.ttf",
  "Geist-SemiBold.ttf",
  "Geist-SemiBoldItalic.ttf",
  "Geist-Bold.ttf",
  "Geist-BoldItalic.ttf",
  "GeistMono.ttf",
  "GeistMono-Medium.ttf",
  "GeistMono-SemiBold.ttf",
];

// The module draws with WebGPU only: GPUI's renderer needs storage
// buffers, which WebGL2 doesn't have (#12237).
// A browser can name WebGPU and still have no adapter for it (a headless
// or blocklisted GPU), so the check asks for one.
function supported() {
  if (!navigator.gpu) {
    return Promise.resolve(false);
  }
  return navigator.gpu.requestAdapter().then(function (adapter) {
    return !!adapter;
  }, function () {
    return false;
  });
}

function say(text) {
  var status = document.getElementById("gpui-status");
  if (status) {
    status.textContent = text;
  }
}

// Without WebGPU the HTML chat takes over: the page goes to
// its address (the note's link), and shows the note if it can't.
function unsupported() {
  var status = document.getElementById("gpui-status");
  if (status) {
    status.remove();
  }
  var note = document.getElementById("gpui-unsupported");
  if (note) {
    note.hidden = false;
    var link = note.querySelector("a");
    if (link) {
      location.replace(link.href);
    }
  }
}

function start() {
  var started = performance.now();
  // The site's shared font files when the page names them (preloads with
  // `data-gpui-font`, in FONTS order), else the build's own copies.
  var shared = document.querySelectorAll("link[data-gpui-font]");
  var urls = shared.length === FONTS.length
    ? Array.prototype.map.call(shared, function (link) { return link.href; })
    : FONTS.map(function (name) { return new URL("fonts/" + name, import.meta.url); });
  var fonts = Promise.all(
    urls.map(function (url, at) {
      var name = FONTS[at];
      return fetch(url).then(function (response) {
        if (!response.ok) {
          throw new Error("font " + name + ": " + response.status);
        }
        return response.arrayBuffer();
      }).then(function (buffer) {
        return new Uint8Array(buffer);
      });
    })
  );
  document.addEventListener("gpui-first-frame", function () {
    var shell = document.getElementById("gpui-shell");
    if (shell) {
      shell.style.display = "none";
    }
    // Measure the actual first frame, including renderer initialization.
    window.__gpuiReadyAt = performance.now();
    window.__gpuiReadyMs = window.__gpuiReadyAt - started;
    document.documentElement.dataset.gpui = "ready";
  }, { once: true });
  Promise.all([init({ module_or_path: WebAssembly.compileStreaming(fetch(new URL("./openagents_web_chat_bg.wasm", import.meta.url))) }), fonts])
    .then(function (loaded) {
      launch(loaded[1]);
    })
    .catch(function (error) {
      say("The chat couldn't start. Reload the page to try again.");
      console.error(error);
    });
}

supported().then(function (ok) {
  if (ok) {
    start();
  } else {
    unsupported();
  }
});
