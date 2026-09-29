"use strict";

function money(cents) {
  return `$${(cents / 100).toFixed(2)}`;
}

function percent(ratio) {
  return `${Math.round(ratio * 100)}%`;
}

function truncate(text, width) {
  return text.length <= width ? text : `${text.slice(0, width - 1)}…`;
}

module.exports = { money, percent, truncate };
