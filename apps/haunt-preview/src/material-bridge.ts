import { CommandQueue } from "./command-queue.ts";
import { MAX_SOURCE_BYTES } from "./protocol.ts";

type Project = { revision: number; materialDraft?: { source: string } };
type Receipt = { revision: number };

type Host = {
  project(): Project;
  submit(source: string, origin: string): Promise<Receipt>;
  preview(source: string): Promise<Record<string, unknown>>;
  write(source: string): Promise<unknown>;
  error(error: unknown): void;
};

const encoder = new TextEncoder();

function explain(error: unknown): string {
  return (error instanceof Error ? error.message : String(error)).replace(
    /^(?:Error:\s*)+/,
    "",
  );
}

function checkSource(source: string): void {
  if (typeof source !== "string") {
    throw new Error("Material source must be text.");
  }
  if (encoder.encode(source).length > MAX_SOURCE_BYTES) {
    throw new Error("Material source is too large.");
  }
}

/**
 * Bridges material source edits to the host's project, preview, and file
 * services. Source commits run through a checked FIFO queue; previews are
 * started by the host's project callback and awaited outside that queue so
 * later edits never wait on an older GPU compilation.
 */
export function MaterialBridge(host: Host) {
  const queue = new CommandQueue();
  const previews = new Map<number, Promise<Record<string, unknown>>>();
  const selfWrites = new Set<string>();
  let knownFile: string | undefined;

  function rememberPreview(revision: number, promise: Promise<Record<string, unknown>>) {
    previews.set(revision, promise);
    if (previews.size > 64) {
      const oldest = previews.keys().next().value;
      if (oldest !== undefined) previews.delete(oldest);
    }
  }

  async function onProject(project: Project): Promise<void> {
    const draft = project.materialDraft;
    if (draft === undefined) return;

    const source = draft.source;
    const revision = project.revision;

    const preview = host.preview(source);
    rememberPreview(revision, preview);

    if (knownFile === source) return;

    selfWrites.add(source);
    try {
      await host.write(source);
      knownFile = source;
    } finally {
      selfWrites.delete(source);
    }
  }

  function save(source: string, origin = "manual"): Promise<Record<string, unknown>> {
    checkSource(source);

    const job = queue.run(async () => {
      const current = host.project();
      const draft = current.materialDraft;
      let revision: number;
      if (draft !== undefined && draft.source === source) {
        await onProject(current);
        revision = current.revision;
      } else {
        const receipt = await host.submit(source, origin);
        revision = receipt.revision;
      }
      return { revision, preview: previews.get(revision) };
    });

    return job.then(async (result) => {
      const preview = result.preview;
      if (preview === undefined) {
        throw new Error("No preview is available for this material revision.");
      }
      return await preview;
    });
  }

  function fileChanged(source: string): void {
    knownFile = source;
    if (selfWrites.has(source)) return;

    let promise: Promise<Record<string, unknown>>;
    try {
      promise = save(source, "file");
    } catch (error) {
      host.error(error);
      return;
    }
    promise.catch((error) => {
      host.error(error);
    });
  }

  return { save, fileChanged, onProject };
}
