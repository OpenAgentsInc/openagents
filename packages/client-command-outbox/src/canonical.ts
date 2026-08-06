import { sha256 } from "@noble/hashes/sha256";
import { bytesToHex, utf8ToBytes } from "@noble/hashes/utils";

import { CommandFingerprint } from "./model.ts";

const SENSITIVE_KEY = /(?:^|[_-])(?:access[_-]?token|api[_-]?key|authorization|bearer|cookie|credential|password|private[_-]?key|refresh[_-]?token|secret|session[_-]?token)(?:$|[_-])/iu;
const SENSITIVE_VALUE = /^(?:bearer\s+|gh[opsu]_[A-Za-z0-9_]{20,}|sk-[A-Za-z0-9_-]{20,}|-----BEGIN [A-Z ]*PRIVATE KEY-----)/u;

export class CommandCanonicalizationError extends Error {
  readonly path: ReadonlyArray<string | number>;

  constructor(message: string, path: ReadonlyArray<string | number>) {
    super(`${message} at ${path.length === 0 ? "$" : path.join(".")}`);
    this.name = "CommandCanonicalizationError";
    this.path = path;
  }
}

export const canonicalCommandJson = (value: unknown): string => {
  const encode = (current: unknown, path: ReadonlyArray<string | number>): string => {
    if (current === null) return "null";

    switch (typeof current) {
      case "boolean":
        return current ? "true" : "false";
      case "number":
        if (!Number.isFinite(current)) throw new CommandCanonicalizationError("non-finite number", path);
        return JSON.stringify(current);
      case "string":
        if (SENSITIVE_VALUE.test(current)) {
          throw new CommandCanonicalizationError("credential-shaped value is not persistable", path);
        }
        return JSON.stringify(current);
      case "object": {
        if (Array.isArray(current)) {
          return `[${current
            .map((item, index) => {
              if (item === undefined) {
                throw new CommandCanonicalizationError("undefined array element", [...path, index]);
              }
              return encode(item, [...path, index]);
            })
            .join(",")}]`;
        }

        const prototype = Object.getPrototypeOf(current);
        if (prototype !== Object.prototype && prototype !== null) {
          throw new CommandCanonicalizationError("non-plain object", path);
        }

        const members: Array<string> = [];
        for (const key of Object.keys(current).sort()) {
          if (SENSITIVE_KEY.test(key)) {
            throw new CommandCanonicalizationError("credential-shaped field is not persistable", [...path, key]);
          }
          const member = Reflect.get(current, key);
          if (member === undefined) continue;
          members.push(`${JSON.stringify(key)}:${encode(member, [...path, key])}`);
        }
        return `{${members.join(",")}}`;
      }
      default:
        throw new CommandCanonicalizationError(`unsupported ${typeof current}`, path);
    }
  };

  return encode(value, []);
};

export const commandFingerprint = (value: unknown): CommandFingerprint =>
  CommandFingerprint.make(`sha256:${bytesToHex(sha256(utf8ToBytes(canonicalCommandJson(value))))}`);
