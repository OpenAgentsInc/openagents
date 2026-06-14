import { describe, expect, test } from "bun:test"
import {
  makeLoopbackPreview,
  servePreviewRequest,
  type CloudDeployResult,
  type LoopbackListener,
  type LoopbackPreviewHandle,
  type SiteBundle,
} from "../src/bun/loopback-preview"

// A fake generated site bundle: an index, a stylesheet, and a binary asset.
const bundle: SiteBundle = {
  files: {
    "/index.html": { contentType: "text/html; charset=utf-8", body: "<!doctype html><h1>hi</h1>" },
    "/assets/app.css": { contentType: "text/css; charset=utf-8", body: "h1{color:red}" },
    "/favicon.ico": { contentType: "image/x-icon", body: new Uint8Array([0, 1, 2, 3]) },
  },
  indexFallback: "/index.html",
}

describe("#4994 servePreviewRequest", () => {
  test("serves a known path with its content-type", () => {
    const res = servePreviewRequest(bundle, "/assets/app.css")
    expect(res.status).toBe(200)
    expect(res.contentType).toBe("text/css; charset=utf-8")
    expect(res.body).toBe("h1{color:red}")
    expect(res.servedPath).toBe("/assets/app.css")
  })

  test("serves binary bodies unchanged", () => {
    const res = servePreviewRequest(bundle, "/favicon.ico")
    expect(res.status).toBe(200)
    expect(res.contentType).toBe("image/x-icon")
    expect(res.body).toEqual(new Uint8Array([0, 1, 2, 3]))
  })

  test("resolves '/' through the index fallback", () => {
    const res = servePreviewRequest(bundle, "/")
    expect(res.status).toBe(200)
    expect(res.servedPath).toBe("/index.html")
    expect(res.contentType).toBe("text/html; charset=utf-8")
  })

  test("an unknown path falls back to index.html (SPA routing)", () => {
    const res = servePreviewRequest(bundle, "/some/client/route")
    expect(res.status).toBe(200)
    expect(res.servedPath).toBe("/index.html")
    expect(res.body).toBe("<!doctype html><h1>hi</h1>")
  })

  test("strips query and hash before routing", () => {
    const res = servePreviewRequest(bundle, "/assets/app.css?v=2#top")
    expect(res.status).toBe(200)
    expect(res.servedPath).toBe("/assets/app.css")
  })

  test("normalizes a path without a leading slash", () => {
    const res = servePreviewRequest(bundle, "assets/app.css")
    expect(res.status).toBe(200)
    expect(res.servedPath).toBe("/assets/app.css")
  })

  test("unknown path with no fallback is a 404", () => {
    const noFallback: SiteBundle = { files: bundle.files }
    const res = servePreviewRequest(noFallback, "/missing")
    expect(res.status).toBe(404)
    expect(res.servedPath).toBeNull()
    expect(res.contentType).toBe("text/plain; charset=utf-8")
  })

  test("404 when the configured fallback target itself is missing", () => {
    const danglingFallback: SiteBundle = {
      files: { "/index.html": { contentType: "text/html", body: "x" } },
      indexFallback: "/does-not-exist.html",
    }
    const res = servePreviewRequest(danglingFallback, "/whatever")
    expect(res.status).toBe(404)
  })

  test("is deterministic for a given (bundle, path)", () => {
    const a = servePreviewRequest(bundle, "/some/route")
    const b = servePreviewRequest(bundle, "/some/route")
    expect(a).toEqual(b)
  })
})

describe("#4994 makeLoopbackPreview", () => {
  // Fake injected listener: records the handler, never opens a socket.
  function fakeListener() {
    const state = {
      started: false,
      stopped: false,
      handle: null as ((p: string) => ReturnType<typeof servePreviewRequest>) | null,
    }
    const listen: LoopbackListener = ({ handle }) => {
      state.started = true
      state.handle = handle
      const out: LoopbackPreviewHandle = {
        url: "http://127.0.0.1:0",
        stop: () => {
          state.stopped = true
        },
      }
      return out
    }
    return { state, listen }
  }

  test("start() wires the pure router behind the injected listener", () => {
    const fake = fakeListener()
    const preview = makeLoopbackPreview({
      bundle,
      listen: fake.listen,
      deployToCloud: async () => ({ accepted: false, url: null, reason: "not called" }),
    })

    const handle = preview.start()
    expect(fake.state.started).toBe(true)
    expect(handle.url).toBe("http://127.0.0.1:0")

    // The handler the fake captured routes through the pure core.
    const routed = fake.state.handle!("/assets/app.css")
    expect(routed.status).toBe(200)
    expect(routed.servedPath).toBe("/assets/app.css")

    handle.stop()
    expect(fake.state.stopped).toBe(true)
  })

  test("serve() exposes the pure router directly (no socket)", () => {
    const fake = fakeListener()
    const preview = makeLoopbackPreview({
      bundle,
      listen: fake.listen,
      deployToCloud: async () => ({ accepted: true, url: null, reason: "ok" }),
    })
    expect(fake.state.started).toBe(false)
    expect(preview.serve("/").servedPath).toBe("/index.html")
    expect(fake.state.started).toBe(false)
  })

  test("deploy() hands the SAVED bundle to the injected cloud seam", async () => {
    const fake = fakeListener()
    let receivedBundle: SiteBundle | null = null
    const cloudResult: CloudDeployResult = {
      accepted: true,
      url: "https://example-site.openagents.com",
      reason: "hosted",
    }
    const preview = makeLoopbackPreview({
      bundle,
      listen: fake.listen,
      deployToCloud: async (input) => {
        receivedBundle = input.bundle
        return cloudResult
      },
    })

    const result = await preview.deploy()
    // The exact bundle the desktop previewed is what gets handed to the cloud.
    expect(receivedBundle).toBe(bundle)
    expect(result).toEqual(cloudResult)
  })
})
