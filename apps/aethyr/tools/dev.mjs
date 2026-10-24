import { createServer } from "vite";
import { devOrigins } from "./dev-origins.mjs";

const origins = devOrigins();
process.env.VITE_EDITOR_ORIGIN ??= origins.editorOrigin;
process.env.VITE_PREVIEW_URL ??= origins.previewOrigin + "/preview.html";
const profile = process.env.VITE_WORLD_PROFILE ?? "medieval-town";
const editor = await createServer({
  server: { port: origins.editorPort },
  cacheDir: `node_modules/.vite-${profile}-editor`,
});
const preview = await createServer({
  server: { port: origins.previewPort },
  cacheDir: `node_modules/.vite-${profile}-preview`,
});
let closing = false;
async function close() {
  if (closing) return;
  closing = true;
  await Promise.all([editor.close(), preview.close()]);
}
process.on("SIGINT", () => void close().then(() => process.exit(0)));
process.on("SIGTERM", () => void close().then(() => process.exit(0)));
try {
  await preview.listen();
  await editor.listen();
  editor.printUrls();
  console.log(`Preview origin: ${origins.previewOrigin}/preview.html`);
} catch (error) {
  await close();
  throw error;
}
