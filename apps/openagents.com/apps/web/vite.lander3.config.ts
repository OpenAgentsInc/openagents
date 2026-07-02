import { defineConfig } from 'vite'

// Second, isolated build for the `/lander3` async hero scene: one
// self-contained ES module (Three.js inlined) emitted into the same asset dir
// the Worker serves. Deliberately NOT part of the main app graph — adding it
// as a second rollup input would re-chunk the main bundle; this keeps the
// production SPA build byte-identical while the experiment loads its scene
// lazily. Unhashed filename: the /lander3 document references it by stable
// path; assets are served with revalidation, and the experiment page is
// no-store.
export default defineConfig({
  build: {
    emptyOutDir: false,
    lib: {
      entry: 'src/scene/lander3-scene-entry.ts',
      fileName: () => 'lander3-scene.js',
      formats: ['es'],
    },
    outDir: 'dist/assets',
  },
  logLevel: 'warn',
})
