// The tool worker: load Kiln's WebAssembly, then run each tool the page posts.
import init, { run_task } from './pkg/kiln.js';

const ready = init();
self.onmessage = async (event) => {
  await ready;
  try {
    self.postMessage(await run_task(event.data));
  } catch (error) {
    self.postMessage(String(error));
  }
};
