export interface InputBatch {
  keys: string[];
  dx: number;
  dy: number;
}

export interface InputBatchHost {
  request(callback: () => void): number;
  cancel(id: number): void;
  send(batch: InputBatch): void;
}

const MAX_KEYS = 32;
const MAX_KEY_LENGTH = 32;
const MAX_DELTA = 4096;

function clampDelta(value: number): number {
  if (value > MAX_DELTA) return MAX_DELTA;
  if (value < -MAX_DELTA) return -MAX_DELTA;
  return value;
}

function sanitizeKeys(keys: readonly string[]): string[] {
  const result: string[] = [];
  for (const key of keys) {
    if (result.length >= MAX_KEYS) break;
    if (typeof key !== "string") continue;
    result.push(key.slice(0, MAX_KEY_LENGTH));
  }
  return result;
}

export class InputBatcher {
  private readonly host: InputBatchHost;
  private keys: string[] = [];
  private dx = 0;
  private dy = 0;
  private frameId: number | null = null;
  private disposed = false;
  private sentCount = 0;

  constructor(host: InputBatchHost) {
    this.host = host;
  }

  get sent(): number {
    return this.sentCount;
  }

  setKeys(keys: readonly string[]): void {
    if (this.disposed) return;
    this.keys = sanitizeKeys(keys);
    this.schedule();
  }

  look(dx: number, dy: number): void {
    if (this.disposed) return;
    if (!Number.isFinite(dx) || !Number.isFinite(dy)) return;
    this.dx = clampDelta(this.dx + dx);
    this.dy = clampDelta(this.dy + dy);
    this.schedule();
  }

  clear(): void {
    if (this.disposed) return;
    this.cancelFrame();
    this.keys = [];
    this.dx = 0;
    this.dy = 0;
    this.sendNow();
  }

  flush(): void {
    if (this.disposed) return;
    this.cancelFrame();
    this.sendNow();
  }

  dispose(): void {
    if (this.disposed) return;
    this.disposed = true;
    this.cancelFrame();
    this.keys = [];
    this.dx = 0;
    this.dy = 0;
  }

  private schedule(): void {
    if (this.disposed || this.frameId !== null) return;
    this.frameId = this.host.request(() => {
      this.frameId = null;
      this.sendNow();
    });
  }

  private cancelFrame(): void {
    if (this.frameId === null) return;
    const id = this.frameId;
    this.frameId = null;
    this.host.cancel(id);
  }

  private sendNow(): void {
    if (this.disposed) return;
    const batch: InputBatch = {
      keys: [...this.keys],
      dx: this.dx,
      dy: this.dy,
    };
    this.dx = 0;
    this.dy = 0;
    this.sentCount += 1;
    this.host.send(batch);
  }
}
