import * as FileSystem from "@effect/platform/FileSystem";
import * as Path from "@effect/platform/Path";
import * as BunContext from "@effect/platform-bun/BunContext";
import * as DefaultServices from "effect/DefaultServices";
import * as Context from "effect/Context";
import * as Layer from "effect/Layer";
import { Effect, Option } from "effect";
import * as Secret from "effect/Secret";
import type { LogLevel, OpenRouterLogger } from "./openrouter-types.js";

const levelOrder: Record<LogLevel, number> = {
  debug: 10,
  info: 20,
  warn: 30,
  error: 40,
};

export const parseLogLevel = (value: string | undefined, fallback: LogLevel): LogLevel => {
  if (!value) return fallback;
  const normalized = value.toLowerCase();
  if (normalized === "debug" || normalized === "info" || normalized === "warn" || normalized === "error") {
    return normalized;
  }
  return fallback;
};

export const logAtLevel = (logger: OpenRouterLogger, level: LogLevel, message: string) => {
  if (levelOrder[level] < levelOrder[logger.level]) return;
  logger[level](message);
};

export const consoleLogger = (level: LogLevel = "warn"): OpenRouterLogger => ({
  level,
  debug: (msg) => console.debug(msg),
  info: (msg) => console.info(msg),
  warn: (msg) => console.warn(msg),
  error: (msg) => console.error(msg),
});

export interface OpenRouterConfigShape {
  apiKey: Secret.Secret;
  baseUrl: string;
  referer: Option.Option<string>;
  siteName: Option.Option<string>;
  timeoutMs: number;
  logLevel: LogLevel;
}

export class OpenRouterConfig extends Context.Tag("OpenRouterConfig")<
  OpenRouterConfig,
  OpenRouterConfigShape
>() {}

export const loadOpenRouterEnv = (): OpenRouterConfigShape => {
  const env = typeof Bun !== "undefined" ? Bun.env : process.env;
  const apiKey = env.OPENROUTER_API_KEY;
  if (!apiKey) {
    throw new Error("Missing OPENROUTER_API_KEY");
  }

  const parsedTimeout = Number(env.OPENROUTER_TIMEOUT_MS);
  const timeoutMs = Number.isFinite(parsedTimeout) && parsedTimeout > 0 ? parsedTimeout : 120_000;

  return {
    apiKey: Secret.fromString(apiKey),
    baseUrl: env.OPENROUTER_BASE_URL || "https://openrouter.ai/api/v1",
    referer: env.OPENROUTER_REFERER ? Option.some(env.OPENROUTER_REFERER) : Option.none(),
    siteName: env.OPENROUTER_SITE_NAME ? Option.some(env.OPENROUTER_SITE_NAME) : Option.none(),
    timeoutMs,
    logLevel: parseLogLevel(env.OPENROUTER_LOG_LEVEL, "warn"),
  };
};

export const resolveLogger = (
  config: OpenRouterConfigShape,
  requestLogLevel?: LogLevel,
  override?: OpenRouterLogger,
): OpenRouterLogger => {
  if (override) {
    return override;
  }
  const level = requestLogLevel ? parseLogLevel(requestLogLevel, config.logLevel) : config.logLevel;
  return consoleLogger(level);
};

export const openRouterConfigLayer = Layer.effect(OpenRouterConfig, Effect.sync(loadOpenRouterEnv));

const parseDotEnv = (contents: string) =>
  contents
    .split("\n")
    .map((line) => line.trim())
    .filter((line) => line && !line.startsWith("#"))
    .reduce<Record<string, string>>((acc, line) => {
      const eq = line.indexOf("=");
      if (eq === -1) return acc;
      const key = line.slice(0, eq).trim();
      const raw = line.slice(eq + 1).trim();
      const unquoted = raw.replace(/^['"]|['"]$/g, "");
      acc[key] = unquoted;
      return acc;
    }, {});

export const dotenvLocalLayer = Layer.scopedDiscard(
  Effect.gen(function* () {
    const fs = yield* FileSystem.FileSystem;
    const path = yield* Path.Path;
    const envPath = path.join(process.cwd(), ".env.local");
    const exists = yield* fs.exists(envPath);
    if (!exists) return;

    const contents = yield* fs.readFileString(envPath);
    const parsed = parseDotEnv(contents);

    for (const [key, value] of Object.entries(parsed)) {
      if (process.env[key] === undefined) {
        process.env[key] = value;
      }
      if (typeof Bun !== "undefined" && Bun.env[key] === undefined) {
        Bun.env[key] = value;
      }
    }
  }),
);

const defaultServicesLayer = Layer.syncContext(() => DefaultServices.liveServices);
const platformLayer = Layer.mergeAll(defaultServicesLayer, BunContext.layer);
const envLayer = dotenvLocalLayer.pipe(Layer.provideMerge(platformLayer));

export const openRouterBaseLayer = Layer.mergeAll(platformLayer, envLayer, openRouterConfigLayer);
