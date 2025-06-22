import { build } from "esbuild"
import path from "path"
import { fileURLToPath } from "url"

const __filename = fileURLToPath(import.meta.url)
const __dirname = path.dirname(__filename)

async function buildSDKBundle() {
  try {
    const result = await build({
      entryPoints: [path.resolve(__dirname, "../../../packages/sdk/src/browser/index.ts")],
      bundle: true,
      format: "esm",
      platform: "browser",
      target: ["es2020"],
      outfile: path.resolve(__dirname, "../public/js/openagents-sdk-browser.js"),
      sourcemap: true,
      minify: false,
      external: [],
      plugins: [
        {
          name: "browser-replacements",
          setup(build) {
            // Replace SparkService with browser stub
            build.onResolve({ filter: /\.\/SparkService\.js$/ }, (args) => {
              if (args.importer.includes("browser/index")) {
                return {
                  path: path.resolve(__dirname, "./stubs/SparkServiceStub.ts"),
                  watchFiles: [path.resolve(__dirname, "./stubs/SparkServiceStub.ts")]
                }
              }
            })

            // Stub out @buildonspark/spark-sdk completely
            build.onResolve({ filter: /^@buildonspark\/spark-sdk$/ }, () => ({
              path: "spark-sdk-stub",
              namespace: "stub"
            }))

            build.onLoad({ filter: /.*/, namespace: "stub" }, () => ({
              contents: "export const SparkWallet = {}",
              loader: "js"
            }))

            // Stub out Nostr crypto functions
            build.onResolve({ filter: /^@openagentsinc\/nostr$/ }, () => ({
              path: "nostr-stub",
              namespace: "stub"
            }))

            build.onLoad({ filter: /^nostr-stub$/, namespace: "stub" }, () => ({
              contents: `
                import { Layer } from "effect"
                
                // Minimal Nostr stubs for browser
                export const CryptoService = {
                  CryptoServiceLive: Layer.empty
                }
                export const EventService = {
                  EventServiceLive: Layer.empty
                }
                export const RelayService = {
                  RelayServiceLive: Layer.empty
                }
                export const WebSocketService = {
                  WebSocketServiceLive: Layer.empty
                }
                export const Nip90Service = {
                  Nip90ServiceLive: Layer.empty,
                  JobStatus: {
                    Pending: "pending",
                    Processing: "processing",
                    Success: "success",
                    Error: "error"
                  }
                }
              `,
              loader: "js"
            }))

            // Stub lightsparkdev packages
            build.onResolve({ filter: /@lightsparkdev/ }, () => ({
              path: "lightspark-stub",
              namespace: "stub"
            }))
          }
        }
      ],
      define: {
        "process.env.NODE_ENV": "\"production\"",
        "global": "globalThis"
      },
      loader: {
        ".ts": "ts",
        ".js": "js"
      },
      resolveExtensions: [".ts", ".js"],
      logLevel: "info"
    })

    console.log("✅ SDK browser bundle built successfully!")
    console.log("📦 Output:", path.resolve(__dirname, "../public/js/openagents-sdk-browser.js"))

    if (result.errors.length > 0) {
      console.error("Build errors:", result.errors)
    }
    if (result.warnings.length > 0) {
      console.warn("Build warnings:", result.warnings)
    }
  } catch (error) {
    console.error("❌ Failed to build SDK browser bundle:", error)
    process.exit(1)
  }
}

buildSDKBundle()
