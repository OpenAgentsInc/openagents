// A fake Playwright `chromium` for unit tests.
//
// Implements only the slice of the Playwright API that `playwright-page.ts`
// uses, deterministically and with NO real browser/network. This is how the
// runner's provision -> capture -> teardown -> artifact-shape path is proven in
// CI without launching chromium (the real path is exercised by run-once /
// demo:login). The fake writes a tiny placeholder video file so the artifact
// flush + (optional) ffmpeg transcode path is exercised honestly.

import { mkdirSync, writeFileSync } from "node:fs";
import { join } from "node:path";

export interface FakeChromiumOptions {
  /** Pages of (selector/text)-driven state, keyed by the URL navigated to. */
  readonly pages?: Record<string, { readonly text?: string; readonly html?: string }>;
  /** Final URL after navigation (simulate a redirect). Keyed by requested url. */
  readonly redirectTo?: Record<string, string>;
}

export function makeFakeChromium(options: FakeChromiumOptions = {}) {
  const pages = options.pages ?? {};
  const redirects = options.redirectTo ?? {};

  const makePage = (baseUrl: string | undefined, videoTmp: string) => {
    let currentUrl = "about:blank";
    const resolve = (url: string) =>
      /^https?:/.test(url) ? url : `${(baseUrl ?? "").replace(/\/$/, "")}${url}`;
    const key = () => {
      // match by pathname-suffix against configured page keys
      for (const k of Object.keys(pages)) if (currentUrl.endsWith(k)) return k;
      return undefined;
    };
    return {
      url: () => currentUrl,
      goto: async (url: string) => {
        const resolved = resolve(url);
        currentUrl = redirects[url] ?? redirects[resolved] ?? resolved;
        return null;
      },
      click: async () => undefined,
      fill: async () => undefined,
      content: async () => pages[key() ?? ""]?.html ?? "<html></html>",
      locator: (_selector: string) => ({
        first: () => ({
          innerText: async () => pages[key() ?? ""]?.text ?? "",
          evaluate: async () => pages[key() ?? ""]?.html ?? "",
          waitFor: async () => undefined,
        }),
        innerText: async () => pages[key() ?? ""]?.text ?? "",
        waitFor: async () => undefined,
      }),
      getByText: (value: string) => ({
        first: () => ({
          waitFor: async () => {
            if (!(pages[key() ?? ""]?.text ?? "").includes(value)) throw new Error("not visible");
          },
        }),
      }),
      waitForURL: async (pred: (u: string) => boolean) => {
        if (!pred(currentUrl)) throw new Error("url predicate not met");
      },
      screenshot: async ({ path }: { path: string }) => {
        writeFileSync(path, Buffer.from("fake-png"));
      },
      video: () => ({
        path: async () => {
          const p = join(videoTmp, "fake.webm");
          writeFileSync(p, Buffer.from("fake-webm-bytes"));
          return p;
        },
      }),
    };
  };

  return {
    launch: async (_opts?: unknown) => {
      let videoTmp = "";
      return {
        newContext: async (ctxOpts?: { recordVideo?: { dir: string }; baseURL?: string }) => {
          videoTmp = ctxOpts?.recordVideo?.dir ?? "";
          if (videoTmp) mkdirSync(videoTmp, { recursive: true });
          const page = makePage(ctxOpts?.baseURL, videoTmp);
          return {
            tracing: {
              start: async () => undefined,
              stop: async ({ path }: { path: string }) => {
                writeFileSync(path, Buffer.from("fake-trace-zip"));
              },
            },
            newPage: async () => page,
            close: async () => undefined,
          };
        },
        close: async () => undefined,
      };
    },
  };
}
