import { build } from "esbuild";
import { execFileSync } from "node:child_process";
import { mkdirSync, readFileSync, writeFileSync, existsSync } from "node:fs";
const output = "../../crates/openagents-web/content/verse-ui";
mkdirSync(output, { recursive: true });
function writeChanged(file, bytes) {
  const next = Buffer.from(bytes);
  if (!existsSync(file) || !readFileSync(file).equals(next))
    writeFileSync(file, next);
}
const result = await build({
  entryPoints: ["src/main.tsx"],
  outfile: `${output}/ui.js`,
  bundle: true,
  write: false,
  minify: true,
  format: "esm",
  target: "es2023",
  // Preserve string contents while keeping generated output free of trailing blanks.
  supported: { "template-literal": false },
  legalComments: "eof",
  banner: { js: `/*!\n${readFileSync("LICENSE.shadcn", "utf8")}\n*/` },
  define: { "process.env.NODE_ENV": '"production"' },
  tsconfig: "tsconfig.json",
});
writeChanged(`${output}/ui.js`, result.outputFiles[0].contents);
const css = execFileSync(process.execPath, [
  "node_modules/@tailwindcss/cli/dist/index.mjs",
  "-i",
  "src/theme.css",
  "--minify",
]);
writeChanged(`${output}/ui.css`, css);
