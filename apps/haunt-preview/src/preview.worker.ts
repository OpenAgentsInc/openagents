import { installPreviewWorker } from "./runtime.ts";

installPreviewWorker(self as unknown as DedicatedWorkerGlobalScope);
