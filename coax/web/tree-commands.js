// Firm reason: 1 (shared command-buffer transport for browser and native SDK hosts).
// Decode completely before the presenter mutates anything. This is the existing
// component backend's in-process command buffer, not another UI language.
globalThis.coaxTreeCommands = (buffer, at, length, decode) => {
  const view = new DataView(buffer, at, length), commands = [];
  let i = 0;
  const byte = () => view.getUint8(i++);
  const short = () => {const n = view.getUint16(i, true); i += 2; return n;};
  const word = () => {const n = view.getUint32(i, true); i += 4; return n;};
  const text = () => {
    const n = word();
    if (n > length - i) throw new Error("Component string exceeds its frame");
    const s = decode(new Uint8Array(buffer, at + i, n)); i += n; return s;
  };
  while (i < length) {
    const op = byte(), c = [op];
    if (op === 1) c.push(short(), text(), text());
    else if (op === 2 || op === 11) c.push(short());
    else if (op === 3) c.push(short(), short(), short());
    else if ([4, 5, 7, 10].includes(op)) c.push(short(), text());
    else if (op === 6) c.push(short(), text(), text());
    else if (op === 9) c.push(text());
    else if (op === 8) {
      c.push(short()); const runs = byte(), paths = [];
      for (let r = 0; r < runs; r++) {
        const layer = byte(), fill = byte(), alpha = short(), stroke = short(), points = short(), xy = [];
        for (let p = 0; p < points; p++) xy.push(word() | 0, word() | 0);
        paths.push([layer, fill, alpha, stroke, xy]);
      }
      c.push(paths);
    } else throw new Error(`Unknown component command ${op}`);
    commands.push(c);
  }
  return commands;
};
