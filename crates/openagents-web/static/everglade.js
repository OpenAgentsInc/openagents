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

  // /verse's opt-in hand input (#12309): on the button's press and not
  // before, the camera and /lander9's hand landmarker, both from this
  // site; each camera frame's points go to the module's verse_hand_frame.
  // verseHands.synth(pose, x, y) feeds made-up hands for checks.
  function hands(glue) {
    var button = document.getElementById("hand-toggle");
    if (!button || !glue.verse_hand_frame) {
      return;
    }
    var note = document.getElementById("hand-note");
    var video = document.getElementById("hand-video");
    var words = note.textContent;
    var session = null;
    function tell(text, show) {
      note.textContent = text;
      note.classList.toggle("show", !!show);
    }
    function pressed(on) {
      button.setAttribute("aria-pressed", String(on));
      button.setAttribute("aria-label", on ? "Turn off hand tracking" : "Turn on hand tracking");
    }
    function stop() {
      var s = session;
      if (!s) return;
      session = null;
      s.on = false;
      if (s.stream) s.stream.getTracks().forEach(function (t) { t.stop(); });
      if (s.landmarker) s.landmarker.close();
      video.srcObject = null;
      glue.verse_hands_on(false);
      pressed(false);
      tell(words, false);
    }
    function start() {
      var s = { on: true };
      session = s;
      pressed(true);
      tell("Starting your camera. " + words, true);
      if (!navigator.mediaDevices || !navigator.mediaDevices.getUserMedia) {
        stop();
        tell("We couldn't find a camera. The keyboard and mouse still work.", true);
        return;
      }
      var model = import("/verse/hands/vision_bundle.mjs").then(function (vision) {
        return vision.FilesetResolver.forVisionTasks("/verse/hands/wasm").then(function (files) {
          function make(delegate) {
            return vision.HandLandmarker.createFromOptions(files, {
              baseOptions: { modelAssetPath: "/verse/hands/hand_landmarker.task", delegate: delegate },
              runningMode: "VIDEO",
              numHands: 2,
              minHandDetectionConfidence: 0.6,
              minHandPresenceConfidence: 0.6,
              minTrackingConfidence: 0.5,
            });
          }
          return make("GPU").catch(function () { return make("CPU"); });
        });
      });
      navigator.mediaDevices
        .getUserMedia({ video: { facingMode: "user", width: { ideal: 640 }, height: { ideal: 480 } }, audio: false })
        .then(function (stream) {
          s.stream = stream;
          if (!s.on) { stop(); return; }
          video.srcObject = stream;
          return video.play().then(function () { return model; }).then(function (landmarker) {
            if (!s.on) { landmarker.close(); return; }
            s.landmarker = landmarker;
            glue.verse_hands_on(true);
            tell("Hold up a hand. " + words, true);
            var buffer = new Float32Array(2 + 2 * 64);
            var last = -1;
            function step() {
              if (!s.on) return;
              if (video.readyState >= 2 && video.currentTime !== last) {
                last = video.currentTime;
                var t = performance.now();
                var found = landmarker.detectForVideo(video, t);
                var n = Math.min(found.landmarks.length, 2);
                buffer[0] = video.videoWidth / video.videoHeight;
                buffer[1] = n;
                for (var h = 0; h < n; h++) {
                  var o = 2 + h * 64, points = found.landmarks[h], side = found.handedness[h];
                  buffer[o] = side && side[0] ? side[0].score : 1;
                  for (var j = 0; j < 21; j++) {
                    buffer[o + 1 + j * 3] = points[j].x;
                    buffer[o + 2 + j * 3] = points[j].y;
                    buffer[o + 3 + j * 3] = points[j].z;
                  }
                }
                glue.verse_hand_frame(buffer.subarray(0, 2 + n * 64), t, innerWidth, innerHeight);
              }
              if (video.requestVideoFrameCallback) video.requestVideoFrameCallback(step);
              else requestAnimationFrame(step);
            }
            step();
          });
        })
        .catch(function (error) {
          if (session !== s) return;
          stop();
          tell(
            error && error.name === "NotAllowedError"
              ? "Camera access is off for this page, so hand tracking can't start. The keyboard and mouse still work."
              : "We couldn't start hand tracking here. The keyboard and mouse still work.",
            true
          );
        });
    }
    button.addEventListener("click", function () { if (session) stop(); else start(); });
    globalThis.verseHands = {
      on: function (on) { glue.verse_hands_on(!!on); },
      synth: function (pose, x, y, pose2, x2, y2) {
        return glue.verse_hand_synth(pose, x, y, pose2 || "", x2 || 0, y2 || 0, performance.now(), innerWidth, innerHeight);
      },
      moves: function () { return glue.verse_hand_moves(); },
    };
  }

  say("Downloading Everglade…");
  Promise.all([import(glue), download()])
    .then(function (loaded) {
      say("Starting Everglade…");
      return loaded[0].default({ module_or_path: loaded[1] }).then(function () {
        return loaded[0];
      });
    })
    .then(function (glue) {
      say("");
      hands(glue);
    })
    .catch(function (error) {
      say(
        "Everglade could not start: " +
          (error && error.message ? error.message : String(error))
      );
    });
})();
