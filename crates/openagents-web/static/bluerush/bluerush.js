// Blue Rush Studios: the water and crabs behind the game cards, and the
// sand garden. Plain canvas 2D. Colors come from the stylesheet's tokens.
(function () {
  "use strict";

  var reduceQuery = window.matchMedia("(prefers-reduced-motion: reduce)");

  // Any CSS color as [r, g, b], through a scratch canvas.
  var scratch = document.createElement("canvas").getContext("2d");
  function rgb(value, fallback) {
    scratch.fillStyle = fallback;
    scratch.fillStyle = (value || "").trim() || fallback;
    var c = scratch.fillStyle;
    if (c.charAt(0) === "#") {
      return [parseInt(c.slice(1, 3), 16), parseInt(c.slice(3, 5), 16), parseInt(c.slice(5, 7), 16)];
    }
    var m = c.match(/[\d.]+/g) || [0, 0, 0];
    return [+m[0], +m[1], +m[2]];
  }
  function token(name, fallback) {
    return rgb(getComputedStyle(document.documentElement).getPropertyValue(name), fallback);
  }
  function css(c, a) {
    return "rgba(" + c[0] + "," + c[1] + "," + c[2] + "," + (a == null ? 1 : a) + ")";
  }
  function mix(a, b, t) {
    return [a[0] + (b[0] - a[0]) * t, a[1] + (b[1] - a[1]) * t, a[2] + (b[2] - a[2]) * t];
  }

  // ---------------------------------------------------------------- water
  function water(canvas) {
    var section = canvas.parentElement;
    var ctx = canvas.getContext("2d");
    var colors = {
      deep: token("--br-water-deep", "#000930"),
      mid: token("--br-water-mid", "#24396e"),
      light: token("--br-water-light", "#93a4b7"),
      sand: token("--br-sand", "#fddab8"),
      foam: token("--br-sand-light", "#fde6cf"),
      crab: token("--br-crab", "#c76639"),
      crabDark: token("--br-crab-dark", "#983400"),
      eye: token("--br-navy-950", "#000930")
    };
    var CELL = 6;
    var BEACH = 46;
    var w = 0, h = 0, dpr = 1, cols = 0, rows = 0;
    var cur, prev, grid, gridCtx, image;
    var visible = false, running = false, last = 0, time = 0, nextDrop = 0;
    var pointer = null;

    var crabs = [
      { x: 0.18, dir: 1, speed: 0, pause: 0.5, phase: 0, size: 17 },
      { x: 0.72, dir: -1, speed: 0, pause: 1.6, phase: 2, size: 14 },
      { x: 0.46, dir: 1, speed: 0, pause: 3.0, phase: 4, size: 12 }
    ];

    function resize() {
      var rect = canvas.getBoundingClientRect();
      if (rect.width < 1 || rect.height < 1) return;
      dpr = Math.min(window.devicePixelRatio || 1, 2);
      w = rect.width;
      h = rect.height;
      canvas.width = Math.round(w * dpr);
      canvas.height = Math.round(h * dpr);
      cols = Math.ceil(w / CELL) + 2;
      rows = Math.ceil(h / CELL) + 2;
      cur = new Float32Array(cols * rows);
      prev = new Float32Array(cols * rows);
      grid = document.createElement("canvas");
      grid.width = cols;
      grid.height = rows;
      gridCtx = grid.getContext("2d");
      image = gridCtx.createImageData(cols, rows);
      // A few rings so the first frame already looks like water.
      for (var i = 0; i < 6; i++) drop(Math.random() * w, Math.random() * (h - BEACH), 3, 1.2);
      for (var s = 0; s < 30; s++) step();
      draw();
    }

    function drop(x, y, radius, strength) {
      var cx = Math.round(x / CELL) + 1, cy = Math.round(y / CELL) + 1;
      for (var dy = -radius; dy <= radius; dy++) {
        for (var dx = -radius; dx <= radius; dx++) {
          var gx = cx + dx, gy = cy + dy;
          if (gx < 1 || gy < 1 || gx >= cols - 1 || gy >= rows - 1) continue;
          var d = Math.sqrt(dx * dx + dy * dy);
          if (d <= radius) cur[gy * cols + gx] += strength * (1 - d / (radius + 1));
        }
      }
    }

    function step() {
      for (var y = 1; y < rows - 1; y++) {
        var row = y * cols;
        for (var x = 1; x < cols - 1; x++) {
          var i = row + x;
          var next = (cur[i - 1] + cur[i + 1] + cur[i - cols] + cur[i + cols]) * 0.5 - prev[i];
          prev[i] = next * 0.982;
        }
      }
      var t = prev; prev = cur; cur = t;
    }

    function shade() {
      var data = image.data;
      for (var y = 0; y < rows; y++) {
        var depth = y / rows;
        var base = mix(colors.deep, colors.mid, 0.25 + depth * 0.6);
        for (var x = 0; x < cols; x++) {
          var i = y * cols + x;
          var slope = 0;
          if (x > 0 && x < cols - 1 && y > 0 && y < rows - 1) {
            slope = (cur[i - 1] - cur[i + 1]) + (cur[i - cols] - cur[i + cols]);
          }
          var swell = Math.sin(x * 0.11 + time * 0.9) * Math.sin(y * 0.15 - time * 0.7);
          var light = Math.max(-1, Math.min(1, slope * 0.9 + swell * 0.08));
          var c = light > 0 ? mix(base, colors.light, light * 0.85) : mix(base, colors.deep, -light * 0.7);
          var o = i * 4;
          data[o] = c[0]; data[o + 1] = c[1]; data[o + 2] = c[2]; data[o + 3] = 255;
        }
      }
      gridCtx.putImageData(image, 0, 0);
    }

    function beach() {
      var top = h - BEACH;
      ctx.fillStyle = css(colors.sand);
      ctx.beginPath();
      ctx.moveTo(0, h);
      for (var x = 0; x <= w + 10; x += 10) {
        ctx.lineTo(x, top + Math.sin(x * 0.02 + time * 0.8) * 3 + Math.sin(x * 0.051) * 2);
      }
      ctx.lineTo(w, h);
      ctx.closePath();
      ctx.fill();
      ctx.strokeStyle = css(colors.foam, 0.9);
      ctx.lineWidth = 3;
      ctx.beginPath();
      for (var fx = 0; fx <= w + 10; fx += 10) {
        var fy = top - 2 + Math.sin(fx * 0.02 + time * 0.8) * 3 + Math.sin(fx * 0.051) * 2;
        if (fx === 0) ctx.moveTo(fx, fy); else ctx.lineTo(fx, fy);
      }
      ctx.stroke();
    }

    function crab(c) {
      var s = c.size;
      var x = c.x * w;
      var y = h - BEACH * 0.42;
      var walk = c.speed > 0 ? Math.sin(c.phase) : 0;
      ctx.save();
      ctx.translate(x, y);
      // shadow
      ctx.fillStyle = "rgba(0,9,48,0.18)";
      ctx.beginPath();
      ctx.ellipse(0, s * 0.75, s * 1.4, s * 0.3, 0, 0, Math.PI * 2);
      ctx.fill();
      // legs
      ctx.strokeStyle = css(colors.crabDark);
      ctx.lineWidth = Math.max(1.5, s * 0.16);
      ctx.lineCap = "round";
      for (var side = -1; side <= 1; side += 2) {
        for (var k = 0; k < 3; k++) {
          var lift = Math.sin(c.phase + k * 2.1 + (side > 0 ? 1 : 0)) * (c.speed > 0 ? 0.25 : 0);
          var bx = side * s * (0.55 + k * 0.18);
          ctx.beginPath();
          ctx.moveTo(bx, s * 0.1);
          ctx.lineTo(bx + side * s * 0.45, s * (0.25 + lift));
          ctx.lineTo(bx + side * s * 0.6, s * (0.75 + lift * 0.6));
          ctx.stroke();
        }
      }
      // claws
      for (var cs = -1; cs <= 1; cs += 2) {
        var clawLift = walk * 0.08 * cs;
        ctx.beginPath();
        ctx.moveTo(cs * s * 0.6, -s * 0.15);
        ctx.lineTo(cs * s * 1.15, -s * (0.55 + clawLift));
        ctx.stroke();
        ctx.fillStyle = css(colors.crab);
        ctx.beginPath();
        ctx.arc(cs * s * 1.25, -s * (0.75 + clawLift), s * 0.36, 0, Math.PI * 2);
        ctx.fill();
        ctx.fillStyle = css(colors.crabDark);
        ctx.beginPath();
        ctx.moveTo(cs * s * 1.25, -s * (0.75 + clawLift));
        ctx.lineTo(cs * s * 1.62, -s * (1.0 + clawLift));
        ctx.lineTo(cs * s * 1.55, -s * (0.68 + clawLift));
        ctx.closePath();
        ctx.fill();
      }
      // body
      ctx.fillStyle = css(colors.crab);
      ctx.strokeStyle = css(colors.crabDark);
      ctx.lineWidth = Math.max(1, s * 0.1);
      ctx.beginPath();
      ctx.ellipse(0, 0, s, s * 0.62, 0, 0, Math.PI * 2);
      ctx.fill();
      ctx.stroke();
      // eyes
      for (var e = -1; e <= 1; e += 2) {
        ctx.strokeStyle = css(colors.crabDark);
        ctx.lineWidth = Math.max(1, s * 0.1);
        ctx.beginPath();
        ctx.moveTo(e * s * 0.28, -s * 0.5);
        ctx.lineTo(e * s * 0.32, -s * 0.85);
        ctx.stroke();
        ctx.fillStyle = css(colors.eye);
        ctx.beginPath();
        ctx.arc(e * s * 0.32, -s * 0.92, s * 0.15, 0, Math.PI * 2);
        ctx.fill();
      }
      ctx.restore();
    }

    function moveCrabs(dt) {
      crabs.forEach(function (c) {
        var px = c.x * w;
        var scared = pointer && Math.abs(pointer.x - px) < 90 && pointer.y > h - BEACH - 120;
        if (scared) {
          c.dir = pointer.x < px ? 1 : -1;
          c.pause = 0;
          c.speed = 150;
        } else if (c.pause > 0) {
          c.pause -= dt;
          c.speed = 0;
          if (c.pause <= 0) {
            c.speed = 25 + Math.random() * 40;
            c.walk = 0.6 + Math.random() * 1.8;
            if (Math.random() < 0.45) c.dir = -c.dir;
          }
        } else {
          c.walk -= dt;
          if (c.walk <= 0) { c.pause = 0.8 + Math.random() * 3; c.speed = 0; }
        }
        if (c.speed > 0) {
          c.x += (c.dir * c.speed * dt) / Math.max(w, 1);
          c.phase += dt * c.speed * 0.35;
          if (c.x < 0.03) { c.x = 0.03; c.dir = 1; }
          if (c.x > 0.97) { c.x = 0.97; c.dir = -1; }
        }
      });
    }

    function draw() {
      if (!w) return;
      shade();
      ctx.setTransform(dpr, 0, 0, dpr, 0, 0);
      ctx.imageSmoothingEnabled = true;
      ctx.drawImage(grid, -CELL, -CELL, cols * CELL, rows * CELL);
      beach();
      crabs.forEach(crab);
    }

    function frame(now) {
      if (!running) return;
      var dt = Math.min(0.05, (now - (last || now)) / 1000);
      last = now;
      time += dt;
      if (time > nextDrop) {
        drop(Math.random() * w, Math.random() * (h - BEACH), 1, 0.9);
        nextDrop = time + 0.5 + Math.random() * 1.2;
      }
      step();
      moveCrabs(dt);
      draw();
      requestAnimationFrame(frame);
    }

    function update() {
      var should = visible && !reduceQuery.matches && !document.hidden;
      if (should && !running) {
        running = true;
        last = 0;
        requestAnimationFrame(frame);
      } else if (!should) {
        running = false;
      }
    }

    section.addEventListener("pointermove", function (event) {
      var rect = canvas.getBoundingClientRect();
      var x = event.clientX - rect.left, y = event.clientY - rect.top;
      if (running && pointer) {
        var moved = Math.abs(x - pointer.x) + Math.abs(y - pointer.y);
        if (moved > 2 && y < h - BEACH) drop(x, y, 2, Math.min(1.4, moved * 0.05));
      }
      pointer = { x: x, y: y };
    });
    section.addEventListener("pointerleave", function () { pointer = null; });
    section.addEventListener("pointerdown", function (event) {
      if (!running) return;
      var rect = canvas.getBoundingClientRect();
      drop(event.clientX - rect.left, event.clientY - rect.top, 4, 2.5);
    });

    new ResizeObserver(function () { resize(); }).observe(canvas);
    new IntersectionObserver(function (entries) {
      visible = entries[0].isIntersecting;
      update();
    }).observe(section);
    document.addEventListener("visibilitychange", update);
    reduceQuery.addEventListener("change", update);
  }

  // ---------------------------------------------------------- sand garden
  function garden(canvas) {
    var ctx = canvas.getContext("2d");
    var mandalaButton = document.getElementById("br-mandala");
    var smoothButton = document.getElementById("br-smooth");
    var colors = {
      sand: token("--br-sand", "#fddab8"),
      light: token("--br-sand-light", "#fde6cf"),
      groove: token("--br-sand-groove", "#d9a47a"),
      shadow: token("--br-sand-shadow", "#b9774c"),
      stone: token("--br-stone", "#4a5468"),
      stoneLight: token("--br-stone-light", "#93a4b7"),
      guide: token("--br-terra", "#c76639")
    };
    var WAYS = 8;
    var TINES = 5;
    var w = 0, h = 0, dpr = 1, unit = 1;
    var layer, layerCtx, grain;
    var strokes = [];
    var stones = [];
    var mandala = false;
    var active = null;
    var down = null;
    var keys = { x: 0, y: 0, raking: false, shown: false };
    var wiping = false;

    // Points are kept relative to the center in units of the short side,
    // so a resize redraws the same garden.
    function toGarden(x, y) { return { x: (x - w / 2) / unit, y: (y - h / 2) / unit }; }
    function toScreen(p) { return { x: w / 2 + p.x * unit, y: h / 2 + p.y * unit }; }
    function copies(p) {
      if (!mandala) return [p];
      var out = [];
      for (var k = 0; k < WAYS; k++) {
        var a = (Math.PI * 2 * k) / WAYS;
        var cos = Math.cos(a), sin = Math.sin(a);
        out.push({ x: p.x * cos - p.y * sin, y: p.x * sin + p.y * cos });
      }
      return out;
    }

    function makeGrain() {
      grain = document.createElement("canvas");
      grain.width = grain.height = 96;
      var g = grain.getContext("2d");
      var img = g.createImageData(96, 96);
      for (var i = 0; i < img.data.length; i += 4) {
        var r = Math.random();
        var dark = r < 0.5;
        var c = dark ? colors.shadow : colors.light;
        img.data[i] = c[0]; img.data[i + 1] = c[1]; img.data[i + 2] = c[2];
        img.data[i + 3] = Math.random() < 0.35 ? (dark ? 26 : 40) : 0;
      }
      g.putImageData(img, 0, 0);
    }

    function freshSand(target, x0, x1) {
      target.fillStyle = css(colors.sand);
      target.fillRect(x0, 0, x1 - x0, h);
      target.fillStyle = target.createPattern(grain, "repeat");
      target.fillRect(x0, 0, x1 - x0, h);
    }

    function resize() {
      var rect = canvas.getBoundingClientRect();
      if (rect.width < 1 || rect.height < 1) return;
      dpr = Math.min(window.devicePixelRatio || 1, 2);
      w = rect.width;
      h = rect.height;
      unit = Math.min(w, h);
      canvas.width = Math.round(w * dpr);
      canvas.height = Math.round(h * dpr);
      layer = document.createElement("canvas");
      layer.width = canvas.width;
      layer.height = canvas.height;
      layerCtx = layer.getContext("2d");
      layerCtx.setTransform(dpr, 0, 0, dpr, 0, 0);
      if (!keys.shown) { keys.x = w / 2; keys.y = h / 2; }
      redraw();
    }

    function redraw() {
      freshSand(layerCtx, 0, w);
      stones.forEach(function (s) { stoneRings(layerCtx, s); });
      strokes.forEach(function (stroke) {
        for (var i = 1; i < stroke.points.length; i++) {
          segment(layerCtx, stroke.points[i - 1], stroke.points[i], stroke.normals[i - 1], stroke.normals[i]);
        }
      });
      compose();
    }

    function tineWidth() { return Math.max(1.6, unit * 0.0055); }
    function tineGap() { return Math.max(4, unit * 0.013); }

    // One rake segment: five parallel grooves, each a shadow, a groove and
    // a lit edge.
    function segment(target, a, b, na, nb) {
      var A = toScreen(a), B = toScreen(b);
      var gap = tineGap(), lw = tineWidth();
      target.lineCap = "round";
      var passes = [
        [colors.shadow, 0.55, lw * 1.25, 0.7],
        [colors.light, 0.9, lw * 0.7, -0.8],
        [colors.groove, 1, lw * 0.8, 0]
      ];
      passes.forEach(function (pass) {
        target.strokeStyle = css(pass[0], pass[1]);
        target.lineWidth = pass[2];
        target.beginPath();
        for (var t = 0; t < TINES; t++) {
          var off = (t - (TINES - 1) / 2) * gap;
          target.moveTo(A.x + na.x * off + pass[3], A.y + na.y * off + pass[3]);
          target.lineTo(B.x + nb.x * off + pass[3], B.y + nb.y * off + pass[3]);
        }
        target.stroke();
      });
    }

    function stoneRings(target, s) {
      var c = toScreen(s);
      var r = s.r * unit;
      var gap = tineGap(), lw = tineWidth();
      [[colors.shadow, 0.5, lw * 1.25, 0.7], [colors.light, 0.9, lw * 0.7, -0.8], [colors.groove, 1, lw * 0.8, 0]].forEach(function (pass) {
        target.strokeStyle = css(pass[0], pass[1]);
        target.lineWidth = pass[2];
        for (var k = 1; k <= 3; k++) {
          target.beginPath();
          target.ellipse(c.x + pass[3], c.y + pass[3], r * 1.05 + k * gap, r * 0.82 + k * gap, s.tilt, 0, Math.PI * 2);
          target.stroke();
        }
      });
    }

    function stone(s) {
      var c = toScreen(s);
      var r = s.r * unit;
      ctx.save();
      ctx.translate(c.x, c.y);
      ctx.rotate(s.tilt);
      ctx.fillStyle = "rgba(0,9,48,0.28)";
      ctx.beginPath();
      ctx.ellipse(r * 0.18, r * 0.22, r * 1.02, r * 0.78, 0, 0, Math.PI * 2);
      ctx.fill();
      var g = ctx.createRadialGradient(-r * 0.35, -r * 0.4, r * 0.1, 0, 0, r * 1.1);
      g.addColorStop(0, css(colors.stoneLight));
      g.addColorStop(0.55, css(mix(colors.stone, colors.stoneLight, 0.35)));
      g.addColorStop(1, css(colors.stone));
      ctx.fillStyle = g;
      ctx.beginPath();
      ctx.ellipse(0, 0, r, r * 0.76, 0, 0, Math.PI * 2);
      ctx.fill();
      ctx.restore();
    }

    function compose() {
      ctx.setTransform(1, 0, 0, 1, 0, 0);
      ctx.drawImage(layer, 0, 0);
      ctx.setTransform(dpr, 0, 0, dpr, 0, 0);
      if (mandala) {
        ctx.strokeStyle = css(colors.guide, 0.22);
        ctx.lineWidth = 1;
        ctx.setLineDash([4, 6]);
        ctx.beginPath();
        for (var k = 0; k < WAYS; k++) {
          var a = (Math.PI * 2 * k) / WAYS;
          ctx.moveTo(w / 2, h / 2);
          ctx.lineTo(w / 2 + Math.cos(a) * unit, h / 2 + Math.sin(a) * unit);
        }
        ctx.stroke();
        ctx.setLineDash([]);
      }
      stones.forEach(stone);
      if (keys.shown) {
        ctx.strokeStyle = css(colors.shadow, 0.9);
        ctx.lineWidth = 2;
        var half = (tineGap() * (TINES - 1)) / 2 + 3;
        ctx.beginPath();
        ctx.arc(keys.x, keys.y, half, 0, Math.PI * 2);
        ctx.stroke();
        if (keys.raking) {
          ctx.fillStyle = css(colors.shadow, 0.35);
          ctx.fill();
        }
      }
    }

    function normal(a, b) {
      var dx = b.x - a.x, dy = b.y - a.y;
      var len = Math.sqrt(dx * dx + dy * dy) || 1;
      return { x: -dy / len, y: dx / len };
    }

    // Starts a stroke (and its mirrored copies) at a screen point.
    function begin(x, y) {
      var p = toGarden(x, y);
      active = copies(p).map(function (q) {
        var stroke = { points: [q], normals: [] };
        strokes.push(stroke);
        return stroke;
      });
    }

    function extend(x, y) {
      if (!active) return;
      var p = toGarden(x, y);
      var lastFirst = active[0].points[active[0].points.length - 1];
      var dx = (p.x - lastFirst.x) * unit, dy = (p.y - lastFirst.y) * unit;
      if (dx * dx + dy * dy < 9) return;
      var next = copies(p);
      active.forEach(function (stroke, i) {
        var pts = stroke.points;
        var a = pts[pts.length - 1], b = next[i];
        var n = normal(a, b);
        if (!stroke.normals.length) stroke.normals.push(n);
        var prevN = stroke.normals[stroke.normals.length - 1];
        // Blend the turn so the grooves stay continuous.
        var blend = { x: (prevN.x + n.x) / 2, y: (prevN.y + n.y) / 2 };
        var bl = Math.sqrt(blend.x * blend.x + blend.y * blend.y) || 1;
        blend = { x: blend.x / bl, y: blend.y / bl };
        stroke.normals[stroke.normals.length - 1] = blend;
        if (pts.length > 1) {
          // Redraw the previous segment's end with the blended normal.
          segment(layerCtx, pts[pts.length - 2], a, stroke.normals[stroke.normals.length - 2], blend);
        }
        pts.push(b);
        stroke.normals.push(n);
        segment(layerCtx, a, b, blend, n);
      });
      compose();
    }

    function end() {
      if (active) {
        strokes = strokes.filter(function (s) { return s.points.length > 1; });
      }
      active = null;
    }

    function placeStone(x, y) {
      var p = toGarden(x, y);
      var r = 0.03 + Math.random() * 0.025;
      var tilt = Math.random() * Math.PI;
      var center = Math.abs(p.x) + Math.abs(p.y) < 0.03;
      (center ? [p] : copies(p)).forEach(function (q, k) {
        var s = { x: q.x, y: q.y, r: r, tilt: tilt + (Math.PI * 2 * k) / WAYS };
        stones.push(s);
        stoneRings(layerCtx, s);
      });
      compose();
    }

    function local(event) {
      var rect = canvas.getBoundingClientRect();
      return { x: event.clientX - rect.left, y: event.clientY - rect.top };
    }

    canvas.addEventListener("pointerdown", function (event) {
      if (event.button > 0) return;
      canvas.setPointerCapture(event.pointerId);
      var p = local(event);
      keys.shown = false;
      down = { x: p.x, y: p.y, moved: false };
    });
    canvas.addEventListener("pointermove", function (event) {
      if (!down) return;
      var p = local(event);
      if (!down.moved) {
        if (Math.abs(p.x - down.x) + Math.abs(p.y - down.y) < 6) return;
        down.moved = true;
        begin(down.x, down.y);
      }
      extend(p.x, p.y);
    });
    function release(event) {
      if (!down) return;
      if (!down.moved && event.type === "pointerup") placeStone(down.x, down.y);
      end();
      down = null;
    }
    canvas.addEventListener("pointerup", release);
    canvas.addEventListener("pointercancel", release);

    canvas.addEventListener("focus", function () { keys.shown = true; compose(); });
    canvas.addEventListener("blur", function () { keys.shown = false; keys.raking = false; end(); compose(); });
    canvas.addEventListener("keydown", function (event) {
      var step = (event.shiftKey ? 0.06 : 0.02) * unit;
      var dx = 0, dy = 0;
      switch (event.key) {
        case "ArrowLeft": dx = -step; break;
        case "ArrowRight": dx = step; break;
        case "ArrowUp": dy = -step; break;
        case "ArrowDown": dy = step; break;
        case " ":
          event.preventDefault();
          if (!keys.raking) { keys.raking = true; begin(keys.x, keys.y); compose(); }
          return;
        case "Enter":
          event.preventDefault();
          placeStone(keys.x, keys.y);
          return;
        default:
          return;
      }
      event.preventDefault();
      keys.shown = true;
      keys.x = Math.max(0, Math.min(w, keys.x + dx));
      keys.y = Math.max(0, Math.min(h, keys.y + dy));
      if (keys.raking) extend(keys.x, keys.y); else compose();
    });
    canvas.addEventListener("keyup", function (event) {
      if (event.key === " ") { keys.raking = false; end(); compose(); }
    });

    mandalaButton.addEventListener("click", function () {
      mandala = !mandala;
      mandalaButton.setAttribute("aria-pressed", String(mandala));
      compose();
    });

    smoothButton.addEventListener("click", function () {
      if (wiping) return;
      strokes = [];
      stones = [];
      if (reduceQuery.matches) { redraw(); return; }
      wiping = true;
      var start = 0, from = 0;
      function sweep(now) {
        if (!start) start = now;
        var t = Math.min(1, (now - start) / 650);
        var to = w * (t * (2 - t));
        freshSand(layerCtx, from, to + 1);
        from = to;
        compose();
        ctx.fillStyle = css(colors.shadow, 0.35);
        ctx.fillRect(to - 3, 0, 3, h);
        if (t < 1) requestAnimationFrame(sweep); else { wiping = false; compose(); }
      }
      requestAnimationFrame(sweep);
    });

    makeGrain();
    new ResizeObserver(function () { resize(); }).observe(canvas);
  }

  function start() {
    var waterCanvas = document.getElementById("br-water");
    var sandCanvas = document.getElementById("br-sand");
    if (waterCanvas && waterCanvas.getContext) water(waterCanvas);
    if (sandCanvas && sandCanvas.getContext) garden(sandCanvas);
  }

  if (document.readyState === "loading") document.addEventListener("DOMContentLoaded", start);
  else start();
})();
