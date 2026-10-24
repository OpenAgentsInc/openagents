type CaptureIntent =
  | {
      kind: "blob";
      resolve: (blob: Blob) => void;
      reject: (error: Error) => void;
    }
  | {
      kind: "pixels";
      resolve: (pixels: ArrayBuffer) => void;
      reject: (error: Error) => void;
    }
  | { kind: "download" };
/** Only an explicit download may create a download. Each flight has one typed result. */
export class CaptureRequests {
  private current?: CaptureIntent & {
    id: string;
    timer: ReturnType<typeof setTimeout>;
  };
  constructor(private readonly timedOut: (id: string) => void = () => {}) {}
  get busy() {
    return !!this.current;
  }
  private hold(id: string, intent: CaptureIntent, timeout: number) {
    if (this.current)
      throw new Error("Let the current screenshot finish first.");
    const timer = setTimeout(() => {
      this.fail(id, new Error("The screenshot took too long. Try again."));
      this.timedOut(id);
    }, timeout);
    this.current = { ...intent, id, timer };
  }
  blob(id: string, timeout = 14000): Promise<Blob> {
    return new Promise((resolve, reject) =>
      this.hold(id, { kind: "blob", resolve, reject }, timeout),
    );
  }
  pixels(id: string, timeout = 14000): Promise<ArrayBuffer> {
    return new Promise((resolve, reject) =>
      this.hold(id, { kind: "pixels", resolve, reject }, timeout),
    );
  }
  download(id: string, timeout = 14000) {
    this.hold(id, { kind: "download" }, timeout);
  }
  complete(
    id: string | undefined,
    blob: Blob,
  ): "blob" | "download" | undefined {
    if (
      !this.current ||
      id !== this.current.id ||
      this.current.kind === "pixels"
    )
      return;
    const held = this.current;
    this.current = undefined;
    clearTimeout(held.timer);
    if (held.kind === "blob") held.resolve(blob);
    return held.kind;
  }
  completePixels(id: string | undefined, pixels: ArrayBuffer): boolean {
    if (
      !this.current ||
      id !== this.current.id ||
      this.current.kind !== "pixels"
    )
      return false;
    const held = this.current;
    this.current = undefined;
    clearTimeout(held.timer);
    held.resolve(pixels);
    return true;
  }
  fail(id: string, error: Error) {
    if (this.current?.id !== id) return;
    const held = this.current;
    this.current = undefined;
    clearTimeout(held.timer);
    if (held.kind !== "download") held.reject(error);
  }
  cancel(error = new Error("The capture was cancelled.")) {
    if (this.current) this.fail(this.current.id, error);
  }
}
