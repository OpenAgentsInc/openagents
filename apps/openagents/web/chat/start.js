// Starts the GPUI web chat (crates/openagents-web/src/pages/gpui_chat.rs).
// It needs WebGPU; without it, the page says so and links to the regular
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
function supported() {
  return !!navigator.gpu;
}

function say(text) {
  var status = document.getElementById("gpui-status");
  if (status) {
    status.textContent = text;
  }
}

function unsupported() {
  var status = document.getElementById("gpui-status");
  if (status) {
    status.remove();
  }
  var note = document.getElementById("gpui-unsupported");
  if (note) {
    note.hidden = false;
  }
}

if (!supported()) {
  unsupported();
} else {
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
  Promise.all([init(), fonts])
    .then(function (loaded) {
      launch(loaded[1]);
      var shell = document.getElementById("gpui-shell");
      if (shell) {
        shell.style.display = "none";
      }
      // How long the page took to draw the canvas, for the size checks.
      window.__gpuiReadyAt = performance.now();
      window.__gpuiReadyMs = window.__gpuiReadyAt - started;
      document.documentElement.dataset.gpui = "ready";
    })
    .catch(function (error) {
      say("The chat couldn't start. Reload the page to try again.");
      console.error(error);
    });
}
