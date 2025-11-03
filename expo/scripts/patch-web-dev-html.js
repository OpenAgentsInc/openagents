#!/usr/bin/env node
const fs = require('fs')
const path = require('path')

function walk(dir, out = []) {
  for (const entry of fs.readdirSync(dir, { withFileTypes: true })) {
    const p = path.join(dir, entry.name)
    if (entry.isDirectory()) walk(p, out)
    else if (entry.isFile() && p.endsWith('.html')) out.push(p)
  }
  return out
}

function patchHtmlFile(file) {
  try {
    let html = fs.readFileSync(file, 'utf8')
    // Ensure the entry script is a module so import.meta is valid
    html = html.replace(
      /<script src="\/_expo\/static\/js\/web\/entry-/g,
      '<script type="module" src="/_expo/static/js/web/entry-'
    )
    // Inject a shim for __ExpoImportMetaRegistry.url used by Expo runtime
    const shim = `<script>(function(){try{var g=globalThis;g.__ExpoImportMetaRegistry=g.__ExpoImportMetaRegistry||{};if(!Object.getOwnPropertyDescriptor(g.__ExpoImportMetaRegistry,'url')){Object.defineProperty(g.__ExpoImportMetaRegistry,'url',{get:function(){try{return (window&&window.location&&window.location.href)||'http://localhost/'}catch(e){return 'http://localhost/'}}, configurable:true});}}catch(e){}})();</script>`
    html = html.replace('</head>', shim + '</head>')
    fs.writeFileSync(file, html, 'utf8')
    process.stdout.write(`[patched] ${file}\n`)
  } catch (e) {
    console.error(`[patch failed] ${file}:`, e)
    process.exitCode = 1
  }
}

const root = process.argv[2] || 'web-dev'
if (!fs.existsSync(root)) {
  console.error(`Directory not found: ${root}`)
  process.exit(1)
}
for (const f of walk(root)) patchHtmlFile(f)

