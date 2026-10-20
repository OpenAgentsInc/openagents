// Paints the app's first frame before the app has loaded. The page carries,
// right after its `#gpui-mount` box, the painter module and a captured
// scene, both gzipped and base64-encoded in inert script elements, and this
// script inline after wasm-bindgen's `--target no-modules` glue.
//
// It paints the inline frame when the box measures exactly what it was
// captured at; at any other size it asks the site for a frame captured at
// the box's size (`data-other`). Without one, the box stays empty, at its
// final size, until the app draws, so the server's map never shows. The snapshot
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
  box.classList.add("is-live");
  // The box's content size in layout pixels, as the app's canvas measures it.
  var measure = function () {
    var style = getComputedStyle(box);
    var rect = box.getBoundingClientRect();
    return {
      w: rect.width - parseFloat(style.borderLeftWidth) - parseFloat(style.borderRightWidth),
      h: rect.height - parseFloat(style.borderTopWidth) - parseFloat(style.borderBottomWidth),
      dpr: window.devicePixelRatio,
    };
  };
  var size = measure();
  // Whether the box still measures what the frame was captured at.
  var fits = function (want) {
    var now = measure();
    return (
      Math.abs(now.w - want.w) <= 0.01 &&
      Math.abs(now.h - want.h) <= 0.01 &&
      Math.abs(now.dpr - want.dpr) <= 0.001
    );
  };
  var gunzip = function (stream) {
    return new Response(stream.pipeThrough(new DecompressionStream("gzip"))).arrayBuffer();
  };
  var bytes = function (element) {
    var text = atob(element.textContent.trim());
    var out = new Uint8Array(text.length);
    for (var i = 0; i < text.length; i++) {
      out[i] = text.charCodeAt(i);
    }
    return gunzip(new Blob([out]).stream());
  };
  var inline = {
    w: Number(scene.dataset.width),
    h: Number(scene.dataset.height),
    dpr: Number(scene.dataset.scale),
  };
  var want;
  var frame;
  if (fits(inline)) {
    // The page carries this size's frame.
    want = inline;
    frame = bytes(scene);
  } else if (scene.dataset.other) {
    // Another size: the site captures it once and keeps it, so this costs
    // a round trip and some 50 KB, still well before the module.
    want = size;
    var address = scene.dataset.other +
      "&w=" + size.w + "&h=" + size.h + "&dpr=" + size.dpr;
    frame = fetch(address).then(function (response) {
      if (!response.ok) {
        throw new Error("first frame " + response.status);
      }
      mark("first-frame-fetched");
      return gunzip(response.body);
    });
  } else {
    // No frame for this size, but the app is coming: keep the box at its
    // final size and empty until the app draws, rather than show the
    // server's map for an instant and swap it out. The start script puts
    // the map back if the app can't start.
    mark("first-frame-skipped");
    return;
  }
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
  Promise.all([bytes(painter), frame])
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
          } else if (!fits(want)) {
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
      // Without a frame the box stays empty until the app draws.
      canvas.remove();
      mark("first-frame-failed");
      console.error(error);
    });
})();
