"use strict";

// Parses "key=value" lines into an object, skipping blanks and comments.
function parse(text) {
  const out = {};
  for (const raw of text.split("\n")) {
    const line = raw.trim();
    if (!line || line.startsWith("#")) continue;
    const at = line.indexOf("=");
    if (at < 0) throw new Error(`no '=' in ${line}`);
    out[line.slice(0, at).trim()] = line.slice(at + 1).trim();
  }
  return out;
}

module.exports = { parse };
