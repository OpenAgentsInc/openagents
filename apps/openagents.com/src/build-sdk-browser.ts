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
          name: "browser-overrides",
          setup(build) {
            // Use browser version of Nostr package
            build.onResolve({ filter: /^@openagentsinc\/nostr$/ }, () => ({
              path: path.resolve(__dirname, "../../../packages/nostr/src/browser.ts")
            }))

            // Replace Node.js crypto-dependent NIPs with browser versions
            build.onResolve({ filter: /nips\/nip04(\.ts|\.js)?$/ }, () => {
              return {
                path: path.resolve(__dirname, "../../../packages/nostr/src/nips/nip04-browser.ts")
              }
            })

            build.onResolve({ filter: /nips\/nip44(\.ts|\.js)?$/ }, () => {
              return {
                path: path.resolve(__dirname, "../../../packages/nostr/src/nips/nip44-browser.ts")
              }
            })

            // Replace SparkService with browser stub
            build.onResolve({ filter: /\.\/SparkService\.js$/ }, (args) => {
              if (args.importer.includes("browser/index")) {
                return {
                  path: path.resolve(__dirname, "./stubs/SparkServiceStub.ts")
                }
              }
            })

            // Stub out @buildonspark/spark-sdk
            build.onResolve({ filter: /^@buildonspark\/spark-sdk$/ }, () => ({
              path: "spark-sdk-stub",
              namespace: "stub"
            }))

            build.onLoad({ filter: /.*/, namespace: "stub" }, () => ({
              contents: "export const SparkWallet = {}",
              loader: "js"
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
        ".tsx": "tsx",
        ".js": "js"
      },
      resolveExtensions: [".ts", ".tsx", ".js", ".json"],
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
