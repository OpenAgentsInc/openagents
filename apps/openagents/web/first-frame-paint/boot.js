// Paints the app's first frame before the app has loaded. The page carries,
// right after its `#gpui-mount` box, the painter module and a captured
// scene, both gzipped and base64-encoded in inert script elements, and this
// script inline after wasm-bindgen's `--target no-modules` glue.
//
// It paints only when the box measures exactly what the scene was captured
// at; otherwise the box stays empty, at its final size, until the app
// draws, so the server's map never shows for an instant. The snapshot
// canvas sits over the app's own canvas until the app announces its first
// frame (`gpui-first-frame`), then goes in that same task. Until then the
// snapshot shows a progress cursor and takes no input.
(function () {
  var box = document.getElementById("gpui-mount");
  var scene = document.getElementById("first-frame-scene");
  var painter = document.getElementById("first-frame-painter");
  // The app draws with WebGPU only (#12237); without it the page keeps its
  // own map, so there is nothing to bridge.
  if (!box || !scene || !painter || !navigator.gpu || !window.DecompressionStream || !window.WebGL2RenderingContext) {
    return;
  }
  var mark = function (name) {
    if (window.performance && performance.mark) {
      performance.mark(name);
    }
  };
  mark("first-frame-start");
  var had = box.classList.contains("is-live");
  box.classList.add("is-live");
  // Whether the box's content measures what the frame was captured at.
  var fits = function () {
    var style = getComputedStyle(box);
    var rect = box.getBoundingClientRect();
    var width = rect.width - parseFloat(style.borderLeftWidth) - parseFloat(style.borderRightWidth);
    var height = rect.height - parseFloat(style.borderTopWidth) - parseFloat(style.borderBottomWidth);
    return (
      Math.abs(width - Number(scene.dataset.width)) <= 0.01 &&
      Math.abs(height - Number(scene.dataset.height)) <= 0.01 &&
      window.devicePixelRatio === Number(scene.dataset.scale)
    );
  };
  if (!fits()) {
    // No frame for this size, but the app is coming: keep the box at its
    // final size and empty until the app draws, rather than show the
    // server's map for an instant and swap it out. The start script puts
    // the map back if the app can't start.
    mark("first-frame-skipped");
    return;
  }
  var bytes = function (element) {
    var text = atob(element.textContent.trim());
    var out = new Uint8Array(text.length);
    for (var i = 0; i < text.length; i++) {
      out[i] = text.charCodeAt(i);
    }
    return new Response(
      new Blob([out]).stream().pipeThrough(new DecompressionStream("gzip")),
    ).arrayBuffer();
  };
  var canvas = document.createElement("canvas");
  canvas.className = "oa-agent-graph-first-frame";
  canvas.setAttribute("aria-hidden", "true");
  // Over the app's canvas, under the toolbar. Set through the CSSOM, as the
  // app styles its own canvas, so the site's stylesheet stays as it is.
  var look = canvas.style;
  look.position = "absolute";
  look.inset = "0";
  look.width = "100%";
  look.height = "100%";
  look.zIndex = "1";
  look.cursor = "progress";
  var app = false;
  // The app's first frame replaces the snapshot.
  box.addEventListener(
    "gpui-first-frame",
    function (event) {
      app = true;
      if (window.__firstFrameShots) {
        window.__firstFrameShots.app = event.target.toDataURL("image/png");
      }
      canvas.remove();
      mark("first-frame-swapped");
    },
    { once: true },
  );
  Promise.all([bytes(painter), bytes(scene)])
    .then(function (ready) {
      if (app) {
        return;
      }
      wasm_bindgen.initSync({ module: ready[0] });
      wasm_bindgen.paint(canvas, new Uint8Array(ready[1]));
      box.insertBefore(canvas, box.firstChild);
      mark("first-frame-painted");
      // A frame captured at one size is wrong at any other: if the box
      // changes (full screen, a turned phone) before the app draws, the
      // snapshot goes and the box waits for the app.
      if (window.ResizeObserver) {
        var watch = new ResizeObserver(function () {
          if (app || !canvas.isConnected) {
            watch.disconnect();
          } else if (!fits()) {
            canvas.remove();
            watch.disconnect();
          }
        });
        watch.observe(box);
      }
      if (window.__firstFrameShots) {
        window.__firstFrameShots.snapshot = canvas.toDataURL("image/png");
      }
      requestAnimationFrame(function () {
        mark("first-frame-presented");
      });
    })
    .catch(function (error) {
      canvas.remove();
      if (!had && !app) {
        box.classList.remove("is-live");
      }
      console.error(error);
    });
})();
