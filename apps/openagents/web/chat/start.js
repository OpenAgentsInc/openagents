// Starts the GPUI web chat (crates/openagents-web/src/pages/gpui_chat.rs).
// It needs WebGPU or WebGL2; without either, the page says so and links to
// the regular chat. Otherwise the module and the fonts download together,
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

function supported() {
  if (navigator.gpu) {
    return true;
  }
  try {
    return !!document.createElement("canvas").getContext("webgl2");
  } catch (error) {
    return false;
  }
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
  var fonts = Promise.all(
    FONTS.map(function (name) {
      return fetch(new URL("fonts/" + name, import.meta.url)).then(function (response) {
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
