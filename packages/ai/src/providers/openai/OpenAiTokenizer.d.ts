import * as Layer from "effect/Layer";
import * as Tokenizer from "../../core/Tokenizer.js";
/**
 * @since 1.0.0
 * @category Constructors
 */
export declare const make: (options: {
    readonly model: string;
}) => Tokenizer.Tokenizer.Service;
/**
 * @since 1.0.0
 * @category Layers
 */
export declare const layer: (options: {
    readonly model: string;
}) => Layer.Layer<Tokenizer.Tokenizer>;
//# sourceMappingURL=OpenAiTokenizer.d.ts.map