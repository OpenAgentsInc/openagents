// GPUI's atlas in the browser, for a page drawn with the shared Zeron
// components (apps/shared/zeron/gpui.coax: /chat). What the Mac app's
// Sources/Zeron.swift does with CoreText, done with the browser's own text:
// Geist and Geist Mono (the page's data-assets font: entries, SIL OFL 1.1)
// loaded as FontFaces, each font's glyphs measured (advances, kerning,
// ascent and descent) and rasterized into one alpha atlas at the device
// pixel ratio, each glyph at GPUI's four quarter-pixel x offsets; Zeron's
// icons (data-assets icons:, their SVGs) drawn into it at twice their
// device size, as GPUI's svg renderer does; then, frame by frame, every
// other grapheme the text uses (gpui.coax's glyph cache) measured and drawn
// on demand, emoji into a second, color atlas. The app's face exports
// coaxial_gpui_* (apps/openagents-web/chat-face.c); chat.wgsl samples the two
// textures. Every decision about where text goes is the app's Coax code;
// this only rasterizes. Based on Zeron by Wing, MIT.
//
// CoreGraphics thickens light text on a dark ground (glyph dilation, five
// levels by the fill's luminance); a canvas can't. The atlas holds LEVELS of
// them (gpui.coax gp_level folds the five onto these), the heavier drawn
// with a hairline stroke of STROKE device pixels, measured against the Mac's
// render (docs/chat.md has the numbers).
const LEVELS = 2;
const STROKE = [0, 0];
const WIDTH = 8192;            // WebGPU's default maxTextureDimension2D
const CELL_EM = 1.6;           // a glyph's cell, in its font's size (Zeron.swift uses 2.4 for five levels)
const PEN_X = 0.2, PEN_Y = 0.7;
const SUBPIXEL = 4, DYN_MONO = 2048, DYN_COLOR = 256;
const FALLBACK = 'system-ui, sans-serif, "Apple Color Emoji", "Segoe UI Emoji", "Noto Color Emoji"';
const family = (weight) => `zr-geist-${weight}`;

export function startGpui({ device, memory, assets, say }) {
  const fontUrls = assets.filter((a) => a.kind === "font").map((a) => a.url);
  const iconsUrl = assets.find((a) => a.kind === "icons")?.url;
  const placeholder = (format) => device.createTexture({ size: [1, 1], format, usage: GPUTextureUsage.TEXTURE_BINDING | GPUTextureUsage.COPY_DST });
  const g = { mono: placeholder("r8unorm"), color: placeholder("rgba8unorm"), built: 0, version: 0 };
  let icons = {};
  const loaded = (async () => {
    await Promise.all(fontUrls.slice(0, 4).map(async (url, weight) => {
      const face = new FontFace(family(weight), await (await fetch(url)).arrayBuffer());
      await face.load();
      document.fonts.add(face);
    }));
    if (iconsUrl) icons = await (await fetch(iconsUrl)).json();
  })().catch((e) => say?.(`gpui: ${e.message}`, true));

  // measurements in logical pixels, at a large size then scaled (CoreText's are unhinted)
  const measure = new OffscreenCanvas(8, 8).getContext("2d");
  measure.fontKerning = "normal";
  const metrics = new Map();   // weight -> { ascent, descent, advance[], kern[] } at 1 px
  function fontMetrics(weight, codes) {
    if (metrics.has(weight)) return metrics.get(weight);
    const n = codes.length, big = 1000;
    measure.font = `${big}px "${family(weight)}"`;
    const m = measure.measureText("H");
    const advance = codes.map((c) => measure.measureText(String.fromCodePoint(c)).width / big);
    const kern = new Float32Array(n * n);
    // kerning as the browser shapes a pair, less the two advances (a monospaced font has none)
    if (weight !== 3) for (let a = 1; a < 95; a++) for (let b = 1; b < 95; b++) {
      const k = measure.measureText(String.fromCodePoint(codes[a], codes[b])).width / big - advance[a] - advance[b];
      if (Math.abs(k) > 0.000001) kern[a * n + b] = k;
    }
    const out = { ascent: m.fontBoundingBoxAscent / big, descent: m.fontBoundingBoxDescent / big, advance, kern };
    metrics.set(weight, out);
    return out;
  }
  // a glyph's coverage drawn white at a pen position, with the level's stroke
  function ink(ctx, text, x, y, level) {
    ctx.fillText(text, x, y);
    if (STROKE[level] > 0) { ctx.lineWidth = STROKE[level]; ctx.strokeText(text, x, y); }
  }

  // The whole atlas for one device pixel ratio, told to the app's state.
  async function build(x, state, dpr) {
    await loaded;
    await document.fonts.ready;
    const glyphs = x.coaxial_gpui_glyphs(), fonts = x.coaxial_gpui_fonts(), count = x.coaxial_gpui_icons();
    const codes = Array.from({ length: glyphs }, (_, i) => x.coaxial_gpui_glyph_code(i));
    const slots = [];
    let y0 = 0;
    for (let f = 0; f < fonts; f++) {
      const size = x.coaxial_gpui_font_size(f), weight = x.coaxial_gpui_font_weight(f);
      const cell = Math.ceil(size * dpr * CELL_EM), cols = Math.floor(WIDTH / cell);
      const rows = Math.ceil((glyphs * SUBPIXEL * LEVELS) / cols);
      slots.push({ size, weight, cell, cols, rows, y0, penX: Math.round(cell * PEN_X), penY: Math.round(cell * PEN_Y) });
      y0 += rows * cell;
    }
    let iconX = 0, iconY = y0, rowH = 0;
    const tiles = [];
    for (let i = 0; i < count; i++) {
      const px = Math.ceil(x.coaxial_gpui_icon_size(i) * dpr * 2);
      if (iconX + px > WIDTH) { iconX = 0; iconY += rowH; rowH = 0; }
      tiles.push([iconX, iconY, px]); iconX += px + 2; rowH = Math.max(rowH, px + 2);
    }
    const dynCell = Math.max(...slots.map((s) => s.cell)), dynY0 = iconY + rowH + 1, dynCols = Math.floor(WIDTH / dynCell);
    const height = Math.min(device.limits.maxTextureDimension2D, dynY0 + Math.ceil(DYN_MONO / dynCols) * dynCell);
    const alpha = new Uint8Array(WIDTH * height);
    const scratch = () => new Float32Array(memory.buffer, x.coaxial_gpui_scratch(state), 7 + glyphs + glyphs * glyphs);
    for (let f = 0; f < fonts; f++) {
      const s = slots[f], m = fontMetrics(s.weight, codes);
      const sc = scratch();
      sc.set([m.ascent * s.size, m.descent * s.size, s.cell, s.penX, s.penY, s.y0, s.cols], 0);
      for (let i = 0; i < glyphs; i++) sc[7 + i] = m.advance[i] * s.size;
      for (let k = 0; k < glyphs * glyphs; k++) sc[7 + glyphs + k] = m.kern[k] * s.size;
      x.coaxial_gpui_set_font(state, f);
      const h = s.rows * s.cell, canvas = new OffscreenCanvas(WIDTH, h), ctx = canvas.getContext("2d", { willReadFrequently: true });
      ctx.font = `${s.size * dpr}px "${family(s.weight)}"`;
      ctx.fontKerning = "none";
      ctx.textBaseline = "alphabetic";
      ctx.fillStyle = ctx.strokeStyle = "#fff";
      for (let level = 0; level < LEVELS; level++) for (let variant = 0; variant < SUBPIXEL; variant++) for (let i = 0; i < glyphs; i++) {
        const c = codes[i];
        if (c === 32 || c === 0xa0) continue;
        const cell = (level * SUBPIXEL + variant) * glyphs + i;
        const cx = (cell % s.cols) * s.cell, cy = Math.floor(cell / s.cols) * s.cell;
        ctx.save();
        ctx.beginPath(); ctx.rect(cx, cy, s.cell, s.cell); ctx.clip();
        ink(ctx, String.fromCodePoint(c), cx + s.penX + variant / SUBPIXEL, cy + s.penY, level);
        ctx.restore();
      }
      const px = ctx.getImageData(0, 0, WIDTH, h).data;
      for (let k = 0, o = s.y0 * WIDTH; k < WIDTH * h; k++) alpha[o + k] = px[k * 4 + 3];
    }
    // Icons: Zeron's SVGs at their tile's size, the alpha kept.
    const nameAt = () => new Uint8Array(memory.buffer, x.coaxial_gpui_bytes(state), 64);
    await Promise.all(tiles.map(async ([tx, ty, px], i) => {
      const n = x.coaxial_gpui_icon_name(state, i);
      const svg = icons[new TextDecoder().decode(nameAt().slice(0, n))];
      if (!svg) return;
      const sized = svg.replace(/<svg\b[^>]*>/, (head) => head.replace(/\s(width|height)="[^"]*"/g, "").replace("<svg", `<svg width="${px}" height="${px}"`));
      const url = URL.createObjectURL(new Blob([sized], { type: "image/svg+xml" }));
      try {
        const img = new Image(); img.src = url; await img.decode();
        const c = new OffscreenCanvas(px, px), ctx = c.getContext("2d", { willReadFrequently: true });
        ctx.drawImage(img, 0, 0, px, px);
        const d = ctx.getImageData(0, 0, px, px).data;
        for (let yy = 0; yy < px; yy++) for (let xx = 0; xx < px; xx++) alpha[(ty + yy) * WIDTH + tx + xx] = d[(yy * px + xx) * 4 + 3];
        x.coaxial_gpui_set_icon(state, i, tx, ty, px, px);
      } finally { URL.revokeObjectURL(url); }
    }));
    const mono = device.createTexture({ size: [WIDTH, height], format: "r8unorm", usage: GPUTextureUsage.TEXTURE_BINDING | GPUTextureUsage.COPY_DST });
    device.queue.writeTexture({ texture: mono }, alpha, { bytesPerRow: WIDTH }, [WIDTH, height]);
    const colorCols = Math.floor(WIDTH / dynCell), colorHeight = Math.ceil(DYN_COLOR / colorCols) * dynCell;
    const color = device.createTexture({ size: [WIDTH, colorHeight], format: "rgba8unorm", usage: GPUTextureUsage.TEXTURE_BINDING | GPUTextureUsage.COPY_DST });
    g.mono.destroy(); g.color.destroy();
    Object.assign(g, { mono, color, dpr, dynCell, penX: Math.round(dynCell * PEN_X), penY: Math.round(dynCell * PEN_Y), slots });
    x.coaxial_gpui_set_atlas(state, WIDTH, height, LEVELS);
    x.coaxial_gpui_set_dynamic(state, dynCell, g.penX, g.penY, dynY0, dynCols, colorCols, WIDTH, colorHeight);
    g.version += 1;
  }

  // The glyph cache: each new grapheme measured (and whether it draws in color), each wanted cell drawn.
  const cellCanvas = new OffscreenCanvas(8, 8);
  const probe = new OffscreenCanvas(40, 40).getContext("2d", { willReadFrequently: true });
  function cluster(x, state, slot) {
    const code = x.coaxial_gpui_cluster(state, slot), n = code & 255, font = code >> 8;
    const text = new TextDecoder().decode(new Uint8Array(memory.buffer, x.coaxial_gpui_bytes(state), n).slice());
    return { text, slot: g.slots[Math.min(Math.max(font, 0), g.slots.length - 1)] };
  }
  function serve(x, state) {
    if (!g.slots) return;
    for (let k = 0, slot; k < 600 && (slot = x.coaxial_gpui_next_cluster(state)) >= 0; k++) {
      const { text, slot: s } = cluster(x, state, slot);
      measure.font = `${s.size}px "${family(s.weight)}", ${FALLBACK}`;
      const advance = measure.measureText(text).width;
      // drawn in red: a color glyph keeps its own colors
      probe.clearRect(0, 0, 40, 40); probe.font = `28px "${family(s.weight)}", ${FALLBACK}`; probe.fillStyle = "#f00"; probe.textBaseline = "alphabetic";
      probe.fillText(text, 4, 30);
      const d = probe.getImageData(0, 0, 40, 40).data;
      let colored = false;
      for (let i = 0; i < d.length && !colored; i += 4) if (d[i + 3] > 64 && (d[i + 1] > 60 || d[i + 2] > 60)) colored = true;
      x.coaxial_gpui_measured(state, slot, advance, colored ? 1 : 0);
    }
    const side = g.dynCell;
    if (cellCanvas.width !== side) { cellCanvas.width = side; cellCanvas.height = side; }
    const ctx = cellCanvas.getContext("2d", { willReadFrequently: true });
    for (let k = 0, cell; k < 3000 && (cell = x.coaxial_gpui_next_cell(state)) >= 0; k++) {
      if (!x.coaxial_gpui_cell(state, cell)) break;
      const [slot, level, variant, isColor, tx, ty] = new Float32Array(memory.buffer, x.coaxial_gpui_scratch(state), 6);
      const { text, slot: s } = cluster(x, state, slot);
      if (ty + side > (isColor > 0.5 ? g.color : g.mono).height) { x.coaxial_gpui_cell_ready(state, cell); continue; }   // past a clamped atlas
      ctx.clearRect(0, 0, side, side);
      ctx.font = `${s.size * g.dpr}px "${family(s.weight)}", ${FALLBACK}`;
      ctx.textBaseline = "alphabetic"; ctx.fillStyle = ctx.strokeStyle = "#fff";
      if (isColor > 0.5) {
        ctx.fillText(text, g.penX, g.penY);
        const d = ctx.getImageData(0, 0, side, side).data;   // straight alpha, as GPUI keeps it
        device.queue.writeTexture({ texture: g.color, origin: [tx, ty] }, d, { bytesPerRow: side * 4 }, [side, side]);
      } else {
        ink(ctx, text, g.penX + variant / SUBPIXEL, g.penY, Math.min(level, LEVELS - 1));
        const d = ctx.getImageData(0, 0, side, side).data, a = new Uint8Array(side * side);
        for (let i = 0; i < a.length; i++) a[i] = d[i * 4 + 3];
        device.queue.writeTexture({ texture: g.mono, origin: [tx, ty] }, a, { bytesPerRow: side }, [side, side]);
      }
      x.coaxial_gpui_cell_ready(state, cell);
    }
  }
  return { g, build, serve };
}
