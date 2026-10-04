// /everglade (#10525): starts the Everglade web build (#10524). The page's
// #everglade container names the wasm-bindgen glue module and its wasm on
// this site (data-module, data-wasm) and where the pinned pack is served
// (data-pack); the server writes those paths, so this loader holds none.
// It imports the glue, calls its default init() export, which compiles the
// wasm and runs the build's start function, and the build draws in the
// container's canvas. Failures are said in #everglade-status.
(function () {
  "use strict";

  var root = document.getElementById("everglade");
  var status = document.getElementById("everglade-status");
  if (!root) {
    return;
  }

  function say(text) {
    if (status) {
      status.textContent = text;
    }
  }

  var glue = root.getAttribute("data-module");
  var wasm = root.getAttribute("data-wasm");
  if (!glue || !wasm || glue.charAt(0) !== "/" || wasm.charAt(0) !== "/") {
    say("Everglade is unavailable.");
    return;
  }

  import(glue)
    .then(function (module) {
      say("Starting Everglade.");
      return module.default({ module_or_path: wasm });
    })
    .then(function () {
      say("");
    })
    .catch(function (error) {
      say(
        "Everglade could not start: " +
          (error && error.message ? error.message : String(error))
      );
    });
})();
