export interface BrowserVideoEncoderOptions {
  width: number;
  height: number;
  frameRate: { numerator: number; denominator: number };
  bitrate?: number;
}
export interface BrowserFilmEncoder {
  /** Sequential output index, including repeated source frames; pixels remain owned by the caller. */
  encode(pixels: ArrayBuffer, frameIndex: number): Promise<Uint8Array>;
  close(): void;
}

/** Self-contained: serialize this function into the trusted local page without module imports. */
export async function createBrowserVideoEncoder(
  options: BrowserVideoEncoderOptions,
): Promise<BrowserFilmEncoder> {
  const width = options?.width,
    height = options?.height,
    numerator = options?.frameRate?.numerator,
    denominator = options?.frameRate?.denominator,
    bitrate = options?.bitrate ?? 16_000_000;
  if (
    !Number.isInteger(width) ||
    !Number.isInteger(height) ||
    width < 2 ||
    width > 1920 ||
    height < 2 ||
    height > 1080 ||
    width % 2 ||
    height % 2 ||
    !Number.isInteger(numerator) ||
    !Number.isInteger(denominator) ||
    numerator < 1 ||
    numerator > 120_000 ||
    denominator < 1 ||
    denominator > 10_000 ||
    numerator / denominator < 1 ||
    numerator / denominator > 60 ||
    !Number.isInteger(bitrate) ||
    bitrate < 1_000_000 ||
    bitrate > 50_000_000
  )
    throw new Error(
      "Choose even film dimensions up to 1920 by 1080, a frame rate up to 60, and a bounded bitrate.",
    );
  if (typeof VideoEncoder === "undefined" || typeof VideoFrame === "undefined")
    throw new Error("This browser cannot encode H.264 video.");
  const config: VideoEncoderConfig = {
    codec: "avc1.64002A",
    width,
    height,
    framerate: numerator / denominator,
    bitrate,
    hardwareAcceleration: "prefer-hardware",
    latencyMode: "realtime",
    avc: { format: "annexb" },
  };
  const supported = await VideoEncoder.isConfigSupported(config);
  if (!supported.supported || supported.config?.avc?.format !== "annexb")
    throw new Error(
      "This browser cannot encode the requested H.264 Annex B video.",
    );
  let encoder: VideoEncoder | undefined,
    closed = false,
    busy = false,
    nextIndex = 0,
    failure: Error | undefined;
  let active:
    | {
        timestamp: number;
        chunks: Uint8Array[];
        bytes: number;
        reject: (error: Error) => void;
      }
    | undefined;
  const lifecycle = {
    stop(reason: unknown) {
      if (closed) return;
      failure = reason instanceof Error ? reason : new Error(String(reason));
      closed = true;
      active?.reject(failure);
      try {
        if (encoder && encoder.state !== "closed") encoder.close();
      } catch {}
    },
  };
  try {
    encoder = new VideoEncoder({
      error(error) {
        lifecycle.stop(error);
      },
      output(chunk) {
        if (closed) return;
        if (
          !active ||
          chunk.timestamp !== active.timestamp ||
          !Number.isInteger(chunk.byteLength) ||
          chunk.byteLength < 4 ||
          active.chunks.length >= 64 ||
          active.bytes + chunk.byteLength > 16 * 1024 * 1024
        ) {
          lifecycle.stop(
            new Error(
              "The video encoder returned an unexpected or oversized frame.",
            ),
          );
          return;
        }
        try {
          const data = new Uint8Array(chunk.byteLength);
          chunk.copyTo(data);
          if (!(
            data[0] === 0 &&
            data[1] === 0 &&
            (data[2] === 1 || (data[2] === 0 && data[3] === 1))
          ))
            throw new Error("The video encoder did not return Annex B data.");
          if (nextIndex === 0 && chunk.type !== "key")
            throw new Error("The video must start with a key frame.");
          active.chunks.push(data);
          active.bytes += data.byteLength;
        } catch (error) {
          lifecycle.stop(error);
        }
      },
    });
    if (failure) throw failure;
    encoder.configure(config);
    if (failure) throw failure;
  } catch (error) {
    lifecycle.stop(error);
    // A synchronous constructor error callback can run before the assignment above.
    try {
      if (encoder && encoder.state !== "closed") encoder.close();
    } catch {}
    throw failure ?? error;
  }
  return {
    async encode(pixels, frameIndex) {
      if (closed) throw failure ?? new Error("The video encoder is closed.");
      if (busy) throw new Error("Let the current video frame finish first.");
      if (
        !(pixels instanceof ArrayBuffer) ||
        pixels.byteLength !== width * height * 4 ||
        !Number.isSafeInteger(frameIndex) ||
        frameIndex !== nextIndex ||
        frameIndex < 0
      )
        throw new Error(
          "Provide one complete RGBA frame at the next output index.",
        );
      const rate = BigInt(numerator),
        scale = BigInt(denominator) * 1_000_000n,
        timestamp = Number((BigInt(frameIndex) * scale + rate / 2n) / rate),
        end = Number((BigInt(frameIndex + 1) * scale + rate / 2n) / rate);
      if (!Number.isSafeInteger(end) || end > 86_400_000_000)
        throw new Error("Keep the film within 24 hours.");
      busy = true;
      let rejectFrame!: (error: Error) => void;
      const rejected = new Promise<never>((_, reject) => {
        rejectFrame = reject;
      });
      // An encoder may call its error callback synchronously from encode/configure.
      void rejected.catch(() => {});
      active = { timestamp, chunks: [], bytes: 0, reject: rejectFrame };
      const current = active;
      const timer = setTimeout(
        () =>
          lifecycle.stop(new Error("The video frame took too long to encode.")),
        30_000,
      );
      let frame: VideoFrame | undefined;
      try {
        frame = new VideoFrame(pixels, {
          format: "RGBA",
          codedWidth: width,
          codedHeight: height,
          timestamp,
          duration: end - timestamp,
          colorSpace: {
            primaries: "bt709",
            transfer: "iec61966-2-1",
            matrix: "rgb",
            fullRange: true,
          },
        });
        try {
          encoder!.encode(frame, {
            keyFrame:
              frameIndex %
                Math.max(1, Math.round((2 * numerator) / denominator)) ===
              0,
          });
        } finally {
          frame.close();
          frame = undefined;
        }
        await Promise.race([encoder!.flush(), rejected]);
        if (failure) throw failure;
        if (!current.chunks.length)
          throw new Error("The video encoder dropped a frame.");
        const bytes = new Uint8Array(current.bytes);
        let offset = 0;
        for (const chunk of current.chunks) {
          bytes.set(chunk, offset);
          offset += chunk.byteLength;
        }
        nextIndex++;
        return bytes;
      } catch (error) {
        lifecycle.stop(error);
        throw failure ?? error;
      } finally {
        frame?.close();
        clearTimeout(timer);
        current.chunks.length = 0;
        if (active === current) active = undefined;
        busy = false;
      }
    },
    close() {
      lifecycle.stop(new Error("The video encoder is closed."));
    },
  };
}
