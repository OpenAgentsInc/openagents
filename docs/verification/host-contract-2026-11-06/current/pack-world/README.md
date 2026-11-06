# Current ordinary browser pack ownership

The current 0.52 realm and all seven current host reactors execute through
the ordinary v1 browser host, without a replacement pack implementation.
All 23 existing observations pass, including reload, device recovery,
engine replacement, final release and the exact 8,388,608-byte retained
data entry. The [source fence](sources.json) passes.

The initial driver polls the ordinary 6,052-byte page with a 4,096-byte
buffer. `cdp_get` refuses that response, so startup ends before Chrome
launches. The [red attempt](red.log) supplies no workload verdict.
The corrected driver uses the existing 65,536-byte page bound and reports
setup errors. It changes no product or acceptance predicate. The
[physical observations](observed.json) and [green log](green.log) pass.
This fixes #12974; complete three-host/default acceptance remains #12869.

Replay with a private ordinary v1 site and a fresh output directory:

```sh
coax/web/tool.sh aethyr/host/check-browser-pack-world.coax \
  "$PWD" /path/to/private/site /tmp/pack-world-new
```

The site contains `index.html`, the actual realm at `test.wasm`, the
current `build/host-*.wasm` reactors, `package/render.json` and the ordinary
Everglade assets. The driver owns its muted headless browser and loopback
server and stops both on exit.
