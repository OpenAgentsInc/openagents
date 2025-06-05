import { Schema } from "@effect/schema";
import { Context } from "effect";
/**
 * Claude Code response for JSON format
 * @since 1.0.0
 */
export const ClaudeCodeJsonResponse = Schema.Struct({
    content: Schema.String,
    model: Schema.String,
    stop_reason: Schema.optional(Schema.String),
    session_id: Schema.optional(Schema.String),
    usage: Schema.optional(Schema.Struct({
        input_tokens: Schema.Number,
        output_tokens: Schema.Number,
        total_tokens: Schema.Number
    }))
});
/**
 * Claude Code client service tag
 * @since 1.0.0
 */
export const ClaudeCodeClient = Context.GenericTag("ai/ClaudeCodeClient");
//# sourceMappingURL=ClaudeCodeClient.js.map