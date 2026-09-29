"use strict";

// Renders an object back to "key=value" lines, keys sorted.
function format(values) {
  return Object.keys(values)
    .sort()
    .map((key) => `${key}=${values[key]}`)
    .join("\n");
}

module.exports = { format };
