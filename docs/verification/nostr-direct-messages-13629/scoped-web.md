# Scoped DOM web delivery

Issue #13629; 2026-11-15.

The ordinary whole-site release stopped at the existing page/room compatibility
gate: the current tree’s Everglade content pin is 3, while production rooms run
2 and seat the preceding 1. No room deployment or gate bypass was used.

The existing `publish-page.coax` owner staged each canonical HTML wrapper as
an isolated demo. Browser SDK assets stay under that release prefix. The DOM
host resolves its Wasm relative to its script; nested shells resolve their DOM
host beside their own script. A root deployment resolves the same `/build/` and
`/coaxial/` URLs it used before. API URLs and account ownership remain rooted
at the same origin. No new product logic lives in the JavaScript glue.

The temporary demo wrappers were exact copies of the canonical source HTML.
They were removed from the checkout after publication; all application behavior
remains in the canonical Coax module. To reproduce, copy the desired source
HTML to `demos/arcade-NAME/index.html` and invoke the existing publisher:

```sh
CLOUDSDK_CONFIG="$HOME/work/.secrets/gcloud-sa-config" coax/web/tool.sh coax/web/publish-page.coax apps/openagents-web demos/arcade-messages/index.html messages arcade-messages
```

After headless browser qualification, the canonical front doors were backed up
under `~/.openagents/releases/arcade-dm-1371/web-before/`, then copied from the
checked stage under `release-lib.sh`’s generation-owned site lock. Only the six
objects below were replaced. The bucket’s fixed runtime assets, world pages,
global COMMIT/PASSED records and room processes were not changed.

| Route | Isolated runtime | HTML SHA-256 |
| --- | --- | --- |
| `/messages` | `/demo/arcade-messages/builds/20261116T023443Z/` | `84722d7f66a0ce11202e80a12ba5d006adf363127a7516ab4f244183aaf00f65` |
| `/messages/index.html` | `/demo/arcade-messages/builds/20261116T023443Z/` | `84722d7f66a0ce11202e80a12ba5d006adf363127a7516ab4f244183aaf00f65` |
| `/chat` | `/demo/arcade-chat/builds/20261116T023914Z/` | `a26dbe69b84defec102c80c843d15502020d5799665cfb6bcbcc1f145386cf31` |
| `/chat/index.html` | `/demo/arcade-chat/builds/20261116T023914Z/` | `a26dbe69b84defec102c80c843d15502020d5799665cfb6bcbcc1f145386cf31` |
| `/feed` | `/demo/arcade-feed/builds/20261116T023618Z/` | `a24663bc258d96cc6af59b34a4967bfff3833d2b338cba0a26c7361406443365` |
| `/feed/index.html` | `/demo/arcade-feed/builds/20261116T023618Z/` | `a24663bc258d96cc6af59b34a4967bfff3833d2b338cba0a26c7361406443365` |

All six canonical responses match the checked local stages byte for byte.
The retained Everglade and Splice Bots response status/hash pairs are unchanged.
Messages starts its scoped `messages.wasm` without errors at the canonical URL;
Chat does the same for `chat-web.wasm`. Feed’s anonymous outer shell is the
existing chat shell; its actual component was checked in a same-origin frame
and renders the expected account-login state without errors. The browser root
path also requests `/build/messages.wasm` successfully after the relative-loader
change. Authenticated native/web/reference exchange is recorded in the README.

Rollback is a front-door restoration from the saved response manifest, retaining
the unchanged fixed SDK assets it references. The isolated runtime names remain
immutable, so existing open clients keep their original code while new visits
take the restored front door. A future qualified whole-site release naturally
replaces these canonical objects from their normal source pages.
