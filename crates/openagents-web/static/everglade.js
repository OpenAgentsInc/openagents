// /everglade (#10525): starts the Everglade web build (#10524). The page's
// #everglade container names the wasm-bindgen glue module and its wasm on
// this site (data-module, data-wasm) and where the pinned pack is served
// (data-pack); the server writes those paths, so this loader holds none.
// It imports the glue, downloads the wasm with progress, calls the glue's
// default init() export with the bytes, which compiles the wasm and runs the
// build's start function, and the build draws in the container's canvas.
// Progress and failures are said in #everglade-status, over the canvas.
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

  var total = Number(root.getAttribute("data-wasm-bytes")) || 0;

  function megabytes(bytes) {
    return (bytes / 1e6).toFixed(1);
  }

  // Fetches the module with progress, so the page shows how the download is
  // going from its first byte. A gzip response's length is the compressed
  // size, so progress counts against the uncompressed size the server
  // writes in data-wasm-bytes.
  function download() {
    return fetch(wasm, { credentials: "same-origin" }).then(function (response) {
      if (!response.ok) {
        throw new Error("the module answered " + response.status);
      }
      if (!response.body || !total) {
        return response.arrayBuffer();
      }
      var reader = response.body.getReader();
      var chunks = [];
      var received = 0;
      function read() {
        return reader.read().then(function (step) {
          if (step.done) {
            var bytes = new Uint8Array(received);
            var offset = 0;
            chunks.forEach(function (chunk) {
              bytes.set(chunk, offset);
              offset += chunk.length;
            });
            return bytes.buffer;
          }
          chunks.push(step.value);
          received += step.value.length;
          var percent = Math.min(99, Math.floor((received * 100) / total));
          say(
            "Downloading Everglade… " + percent + "% (" + megabytes(received) +
              " of " + megabytes(total) + " MB)"
          );
          return read();
        });
      }
      return read();
    });
  }

  say("Downloading Everglade…");
  Promise.all([import(glue), download()])
    .then(function (loaded) {
      say("Starting Everglade…");
      return loaded[0].default({ module_or_path: loaded[1] });
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
