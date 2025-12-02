import * as S from "effect/Schema";

export interface PartialParseResult<A> {
  decoded: A | null;
  error: string | null;
  raw: string;
}

/**
 * PartialToolArgsParser incrementally accumulates streamed JSON argument text
 * and attempts to decode it against a Schema when the buffer becomes valid JSON.
 */
export class PartialToolArgsParser<A> {
  private buffer = "";
  private latest: A | null = null;
  private lastError: string | null = null;

  constructor(private readonly schema: S.Schema<A>) {}

  append(chunk: string): PartialParseResult<A> {
    this.buffer += chunk;

    const parsed = this.tryParseJSON(this.buffer);
    if (parsed._tag === "Left") {
      this.lastError = parsed.left;
      return { decoded: this.latest, error: this.lastError, raw: this.buffer };
    }

    const decoded = this.tryDecode(parsed.right);
    if (decoded._tag === "Left") {
      this.lastError = decoded.left;
      return { decoded: this.latest, error: this.lastError, raw: this.buffer };
    }

    this.latest = decoded.right;
    this.lastError = null;
    return { decoded: this.latest, error: null, raw: this.buffer };
  }

  get value(): A | null {
    return this.latest;
  }

  get raw(): string {
    return this.buffer;
  }

  private tryParseJSON(input: string): { _tag: "Right"; right: unknown } | { _tag: "Left"; left: string } {
    try {
      return { _tag: "Right", right: JSON.parse(input) };
    } catch (error) {
      return { _tag: "Left", left: (error as Error).message };
    }
  }

  private tryDecode(input: unknown): { _tag: "Right"; right: A } | { _tag: "Left"; left: string } {
    try {
      const value = S.decodeUnknownSync(this.schema)(input);
      return { _tag: "Right", right: value };
    } catch (error) {
      return { _tag: "Left", left: (error as Error).message };
    }
  }
}
