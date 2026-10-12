#!/bin/sh
# The small, real repository Coder works on in terminal-coder.mp4: a slug
# library whose tests fail for three honest bugs (accents, repeated dashes,
# a trailing dash after cutting).
# Usage: make-demo-repo.sh DIR
set -eu
dir=${1:?DIR is required}
rm -rf "$dir"
mkdir -p "$dir/src" "$dir/test"
cd "$dir"
cat > package.json <<'JSON'
{
  "name": "slugkit",
  "version": "0.3.1",
  "description": "Turn titles into URL slugs.",
  "type": "module",
  "main": "src/slugify.js",
  "scripts": { "test": "node --test" },
  "license": "MIT"
}
JSON
cat > src/slugify.js <<'JS'
// Turn a title into a URL slug: "Hello, World!" -> "hello-world".
export function slugify(title, { maxLength = 60 } = {}) {
  const slug = title
    .toLowerCase()
    .replace(/[^a-z0-9]/g, "-");
  return slug.slice(0, maxLength);
}
JS
cat > test/slugify.test.js <<'JS'
import { test } from "node:test";
import assert from "node:assert/strict";
import { slugify } from "../src/slugify.js";

test("lowercases and joins words", () => {
  assert.equal(slugify("Hello World"), "hello-world");
});

test("drops punctuation", () => {
  assert.equal(slugify("Hello, World!"), "hello-world");
});

test("keeps accented letters readable", () => {
  assert.equal(slugify("Crème brûlée recipes"), "creme-brulee-recipes");
});

test("never ends with a dash after trimming", () => {
  assert.equal(slugify("Ship it today", { maxLength: 8 }), "ship-it");
});
JS
cat > README.md <<'MD'
# slugkit

Turn titles into URL slugs.

```js
import { slugify } from "slugkit";
slugify("Crème brûlée recipes"); // "creme-brulee-recipes"
```
MD
git init -q -b main
git add -A
git -c user.name=Alex -c user.email=alex@example.com commit -qm "slugkit 0.3.1"
