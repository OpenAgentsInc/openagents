# openagents.com on Coaxial

The website, drawn by Coax on WebGPU. The framework is specified in
[coax/web.md](../../coax/web.md).

Today this is the splash: `splash.coax` lays out and animates the words,
`graph.coax` is the iOS home's graph (from `metareal/home.coax`, same seed)
fading in behind them as a wide band, `splash.wgsl` draws both over the
nebula, `face.c` exports the Coax
procedures to the browser, and `index.html` / `index.md` carry the same
words as text for search engines, screen readers and browsers without
WebGPU.

```sh
coax/bin/coax test apps/openagents-web/splash.coax   # the splash's checks
coax/web/serve.sh apps/openagents-web                # http://127.0.0.1:8800/, hot reload
apps/openagents-web/build.sh                         # release build: build/splash.wasm
```

`coax serve` rebuilds on save: a `.coax` or `.c` change swaps the engine
into the running page at the next frame and keeps its state (the animation
carries on); a `.wgsl` change swaps the shaders; an `.html`, `.js` or `.md`
change reloads the page. Build errors show in the page's corner with the
`.coax` location; the page keeps running the last good build.

`http://127.0.0.1:8800/?probe` reads one frame back from the GPU and prints
its middle to the console, for checking a render in a headless browser.

A new `.coax` file is watched from the next `coax serve` start; until then,
save `splash.coax` to rebuild.

Needs upstream LLVM: `brew install llvm lld wasi-libc wasi-runtimes`.
