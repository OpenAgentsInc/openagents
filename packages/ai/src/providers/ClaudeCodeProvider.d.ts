import { Layer } from "effect";
import { AiService } from "../AiService.js";
import { ClaudeCodeConfig } from "../config/ClaudeCodeConfig.js";
/**
 * Claude Code provider for AI Service
 * @since 1.0.0
 */
export declare const ClaudeCodeProviderLive: Layer.Layer<AiService, never, import("@effect/platform/FileSystem").FileSystem>;
/**
 * Create a Claude Code provider with custom configuration
 * @since 1.0.0
 */
export declare const makeClaudeCodeProvider: (config: Partial<ClaudeCodeConfig>) => Layer.Layer<AiService, never, import("@effect/platform/FileSystem").FileSystem>;
//# sourceMappingURL=ClaudeCodeProvider.d.ts.map