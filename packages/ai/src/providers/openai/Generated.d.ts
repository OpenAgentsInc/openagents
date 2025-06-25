/**
 * @since 1.0.0
 */
import type * as HttpClient from "@effect/platform/HttpClient";
import * as HttpClientError from "@effect/platform/HttpClientError";
import * as Effect from "effect/Effect";
import type { ParseError } from "effect/ParseResult";
import * as S from "effect/Schema";
declare const ListAssistantsParamsOrder_base: S.Literal<["asc", "desc"]>;
export declare class ListAssistantsParamsOrder extends ListAssistantsParamsOrder_base {
}
declare const ListAssistantsParams_base: S.Struct<{
    limit: S.optionalWith<typeof S.Int, {
        nullable: true;
        default: () => 20;
    }>;
    order: S.optionalWith<typeof ListAssistantsParamsOrder, {
        nullable: true;
        default: () => "desc";
    }>;
    after: S.optionalWith<typeof S.String, {
        nullable: true;
    }>;
    before: S.optionalWith<typeof S.String, {
        nullable: true;
    }>;
}>;
export declare class ListAssistantsParams extends ListAssistantsParams_base {
}
declare const AssistantObjectObject_base: S.Literal<["assistant"]>;
export declare class AssistantObjectObject extends AssistantObjectObject_base {
}
declare const AssistantToolsCodeType_base: S.Literal<["code_interpreter"]>;
export declare class AssistantToolsCodeType extends AssistantToolsCodeType_base {
}
declare const AssistantToolsCode_base: S.Struct<{
    type: typeof AssistantToolsCodeType;
}>;
export declare class AssistantToolsCode extends AssistantToolsCode_base {
}
declare const AssistantToolsFileSearchType_base: S.Literal<["file_search"]>;
export declare class AssistantToolsFileSearchType extends AssistantToolsFileSearchType_base {
}
declare const FileSearchRanker_base: S.Literal<["auto", "default_2024_08_21"]>;
export declare class FileSearchRanker extends FileSearchRanker_base {
}
declare const FileSearchRankingOptions_base: S.Struct<{
    ranker: S.optionalWith<typeof FileSearchRanker, {
        nullable: true;
    }>;
    score_threshold: S.filter<S.filter<typeof S.Number>>;
}>;
export declare class FileSearchRankingOptions extends FileSearchRankingOptions_base {
}
declare const AssistantToolsFileSearch_base: S.Struct<{
    type: typeof AssistantToolsFileSearchType;
    file_search: S.optionalWith<S.Struct<{
        max_num_results: S.optionalWith<S.filter<S.filter<typeof S.Int>>, {
            nullable: true;
        }>;
        ranking_options: S.optionalWith<typeof FileSearchRankingOptions, {
            nullable: true;
        }>;
    }>, {
        nullable: true;
    }>;
}>;
export declare class AssistantToolsFileSearch extends AssistantToolsFileSearch_base {
}
declare const AssistantToolsFunctionType_base: S.Literal<["function"]>;
export declare class AssistantToolsFunctionType extends AssistantToolsFunctionType_base {
}
declare const FunctionParameters_base: S.Record$<typeof S.String, typeof S.Unknown>;
export declare class FunctionParameters extends FunctionParameters_base {
}
declare const FunctionObject_base: S.Struct<{
    description: S.optionalWith<typeof S.String, {
        nullable: true;
    }>;
    name: typeof S.String;
    parameters: S.optionalWith<typeof FunctionParameters, {
        nullable: true;
    }>;
    strict: S.optionalWith<typeof S.Boolean, {
        nullable: true;
        default: () => false;
    }>;
}>;
export declare class FunctionObject extends FunctionObject_base {
}
declare const AssistantToolsFunction_base: S.Struct<{
    type: typeof AssistantToolsFunctionType;
    function: typeof FunctionObject;
}>;
export declare class AssistantToolsFunction extends AssistantToolsFunction_base {
}
declare const Metadata_base: S.Record$<typeof S.String, typeof S.Unknown>;
export declare class Metadata extends Metadata_base {
}
declare const AssistantsApiResponseFormatOptionEnum_base: S.Literal<["auto"]>;
export declare class AssistantsApiResponseFormatOptionEnum extends AssistantsApiResponseFormatOptionEnum_base {
}
declare const ResponseFormatTextType_base: S.Literal<["text"]>;
export declare class ResponseFormatTextType extends ResponseFormatTextType_base {
}
declare const ResponseFormatText_base: S.Struct<{
    type: typeof ResponseFormatTextType;
}>;
export declare class ResponseFormatText extends ResponseFormatText_base {
}
declare const ResponseFormatJsonObjectType_base: S.Literal<["json_object"]>;
export declare class ResponseFormatJsonObjectType extends ResponseFormatJsonObjectType_base {
}
declare const ResponseFormatJsonObject_base: S.Struct<{
    type: typeof ResponseFormatJsonObjectType;
}>;
export declare class ResponseFormatJsonObject extends ResponseFormatJsonObject_base {
}
declare const ResponseFormatJsonSchemaType_base: S.Literal<["json_schema"]>;
export declare class ResponseFormatJsonSchemaType extends ResponseFormatJsonSchemaType_base {
}
declare const ResponseFormatJsonSchemaSchema_base: S.Record$<typeof S.String, typeof S.Unknown>;
export declare class ResponseFormatJsonSchemaSchema extends ResponseFormatJsonSchemaSchema_base {
}
declare const ResponseFormatJsonSchema_base: S.Struct<{
    type: typeof ResponseFormatJsonSchemaType;
    json_schema: S.Struct<{
        description: S.optionalWith<typeof S.String, {
            nullable: true;
        }>;
        name: typeof S.String;
        schema: S.optionalWith<typeof ResponseFormatJsonSchemaSchema, {
            nullable: true;
        }>;
        strict: S.optionalWith<typeof S.Boolean, {
            nullable: true;
            default: () => false;
        }>;
    }>;
}>;
export declare class ResponseFormatJsonSchema extends ResponseFormatJsonSchema_base {
}
declare const AssistantsApiResponseFormatOption_base: S.Union<[typeof AssistantsApiResponseFormatOptionEnum, typeof ResponseFormatText, typeof ResponseFormatJsonObject, typeof ResponseFormatJsonSchema]>;
export declare class AssistantsApiResponseFormatOption extends AssistantsApiResponseFormatOption_base {
}
declare const AssistantObject_base: S.Struct<{
    id: typeof S.String;
    object: typeof AssistantObjectObject;
    created_at: typeof S.Int;
    name: S.NullOr<S.filter<typeof S.String>>;
    description: S.NullOr<S.filter<typeof S.String>>;
    model: typeof S.String;
    instructions: S.NullOr<S.filter<typeof S.String>>;
    tools: S.PropertySignature<":", readonly ({
        readonly type: "code_interpreter";
    } | {
        readonly type: "file_search";
        readonly file_search?: {
            readonly max_num_results?: number | undefined;
            readonly ranking_options?: {
                readonly ranker?: "auto" | "default_2024_08_21" | undefined;
                readonly score_threshold: number;
            } | undefined;
        } | undefined;
    } | {
        readonly function: {
            readonly description?: string | undefined;
            readonly name: string;
            readonly parameters?: {
                readonly [x: string]: unknown;
            } | undefined;
            readonly strict: boolean;
        };
        readonly type: "function";
    })[], never, ":", readonly ({
        readonly type: "code_interpreter";
    } | {
        readonly type: "file_search";
        readonly file_search?: {
            readonly max_num_results?: number | null | undefined;
            readonly ranking_options?: {
                readonly score_threshold: number;
                readonly ranker?: "auto" | "default_2024_08_21" | null | undefined;
            } | null | undefined;
        } | null | undefined;
    } | {
        readonly function: {
            readonly name: string;
            readonly description?: string | null | undefined;
            readonly parameters?: {
                readonly [x: string]: unknown;
            } | null | undefined;
            readonly strict?: boolean | null | undefined;
        };
        readonly type: "function";
    })[], true, never>;
    tool_resources: S.optionalWith<S.Struct<{
        code_interpreter: S.optionalWith<S.Struct<{
            file_ids: S.optionalWith<S.filter<S.Array$<typeof S.String>>, {
                nullable: true;
                default: () => readonly [];
            }>;
        }>, {
            nullable: true;
        }>;
        file_search: S.optionalWith<S.Struct<{
            vector_store_ids: S.optionalWith<S.filter<S.Array$<typeof S.String>>, {
                nullable: true;
            }>;
        }>, {
            nullable: true;
        }>;
    }>, {
        nullable: true;
    }>;
    metadata: S.NullOr<typeof Metadata>;
    temperature: S.optionalWith<S.filter<S.filter<typeof S.Number>>, {
        nullable: true;
        default: () => 1;
    }>;
    top_p: S.optionalWith<S.filter<S.filter<typeof S.Number>>, {
        nullable: true;
        default: () => 1;
    }>;
    response_format: S.optionalWith<typeof AssistantsApiResponseFormatOption, {
        nullable: true;
    }>;
}>;
export declare class AssistantObject extends AssistantObject_base {
}
declare const ListAssistantsResponse_base: S.Class<ListAssistantsResponse, {
    object: typeof S.String;
    data: S.Array$<typeof AssistantObject>;
    first_id: typeof S.String;
    last_id: typeof S.String;
    has_more: typeof S.Boolean;
}, S.Struct.Encoded<{
    object: typeof S.String;
    data: S.Array$<typeof AssistantObject>;
    first_id: typeof S.String;
    last_id: typeof S.String;
    has_more: typeof S.Boolean;
}>, never, {
    readonly object: string;
} & {
    readonly first_id: string;
} & {
    readonly last_id: string;
} & {
    readonly has_more: boolean;
} & {
    readonly data: readonly {
        readonly object: "assistant";
        readonly model: string;
        readonly description: string | null;
        readonly name: string | null;
        readonly id: string;
        readonly created_at: number;
        readonly instructions: string | null;
        readonly tools: readonly ({
            readonly type: "code_interpreter";
        } | {
            readonly type: "file_search";
            readonly file_search?: {
                readonly max_num_results?: number | undefined;
                readonly ranking_options?: {
                    readonly ranker?: "auto" | "default_2024_08_21" | undefined;
                    readonly score_threshold: number;
                } | undefined;
            } | undefined;
        } | {
            readonly function: {
                readonly description?: string | undefined;
                readonly name: string;
                readonly parameters?: {
                    readonly [x: string]: unknown;
                } | undefined;
                readonly strict: boolean;
            };
            readonly type: "function";
        })[];
        readonly tool_resources?: {
            readonly code_interpreter?: {
                readonly file_ids: readonly string[];
            } | undefined;
            readonly file_search?: {
                readonly vector_store_ids?: readonly string[] | undefined;
            } | undefined;
        } | undefined;
        readonly metadata: {
            readonly [x: string]: unknown;
        } | null;
        readonly temperature: number;
        readonly top_p: number;
        readonly response_format?: "auto" | {
            readonly type: "text";
        } | {
            readonly type: "json_object";
        } | {
            readonly type: "json_schema";
            readonly json_schema: {
                readonly description?: string | undefined;
                readonly name: string;
                readonly strict: boolean;
                readonly schema?: {
                    readonly [x: string]: unknown;
                } | undefined;
            };
        } | undefined;
    }[];
}, {}, {}>;
export declare class ListAssistantsResponse extends ListAssistantsResponse_base {
}
declare const AssistantSupportedModels_base: S.Literal<["o3-mini", "o3-mini-2025-01-31", "o1", "o1-2024-12-17", "gpt-4o", "gpt-4o-2024-11-20", "gpt-4o-2024-08-06", "gpt-4o-2024-05-13", "gpt-4o-mini", "gpt-4o-mini-2024-07-18", "gpt-4.5-preview", "gpt-4.5-preview-2025-02-27", "gpt-4-turbo", "gpt-4-turbo-2024-04-09", "gpt-4-0125-preview", "gpt-4-turbo-preview", "gpt-4-1106-preview", "gpt-4-vision-preview", "gpt-4", "gpt-4-0314", "gpt-4-0613", "gpt-4-32k", "gpt-4-32k-0314", "gpt-4-32k-0613", "gpt-3.5-turbo", "gpt-3.5-turbo-16k", "gpt-3.5-turbo-0613", "gpt-3.5-turbo-1106", "gpt-3.5-turbo-0125", "gpt-3.5-turbo-16k-0613"]>;
export declare class AssistantSupportedModels extends AssistantSupportedModels_base {
}
declare const ReasoningEffort_base: S.Literal<["low", "medium", "high"]>;
export declare class ReasoningEffort extends ReasoningEffort_base {
}
declare const CreateAssistantRequest_base: S.Class<CreateAssistantRequest, {
    model: S.Union<[typeof S.String, typeof AssistantSupportedModels]>;
    name: S.optionalWith<S.filter<typeof S.String>, {
        nullable: true;
    }>;
    description: S.optionalWith<S.filter<typeof S.String>, {
        nullable: true;
    }>;
    instructions: S.optionalWith<S.filter<typeof S.String>, {
        nullable: true;
    }>;
    reasoning_effort: S.optionalWith<typeof ReasoningEffort, {
        nullable: true;
        default: () => "medium";
    }>;
    tools: S.optionalWith<S.filter<S.Array$<S.Union<[typeof AssistantToolsCode, typeof AssistantToolsFileSearch, typeof AssistantToolsFunction]>>>, {
        nullable: true;
        default: () => readonly [];
    }>;
    tool_resources: S.optionalWith<S.Struct<{
        code_interpreter: S.optionalWith<S.Struct<{
            file_ids: S.optionalWith<S.filter<S.Array$<typeof S.String>>, {
                nullable: true;
                default: () => readonly [];
            }>;
        }>, {
            nullable: true;
        }>;
        file_search: S.optionalWith<S.Struct<{
            vector_store_ids: S.optionalWith<S.filter<S.Array$<typeof S.String>>, {
                nullable: true;
            }>;
            vector_stores: S.optionalWith<S.filter<S.Array$<S.Struct<{
                file_ids: S.optionalWith<S.filter<S.Array$<typeof S.String>>, {
                    nullable: true;
                }>;
                chunking_strategy: S.optionalWith<S.Record$<typeof S.String, typeof S.Unknown>, {
                    nullable: true;
                }>;
                metadata: S.optionalWith<typeof Metadata, {
                    nullable: true;
                }>;
            }>>>, {
                nullable: true;
            }>;
        }>, {
            nullable: true;
        }>;
    }>, {
        nullable: true;
    }>;
    metadata: S.optionalWith<typeof Metadata, {
        nullable: true;
    }>;
    temperature: S.optionalWith<S.filter<S.filter<typeof S.Number>>, {
        nullable: true;
        default: () => 1;
    }>;
    top_p: S.optionalWith<S.filter<S.filter<typeof S.Number>>, {
        nullable: true;
        default: () => 1;
    }>;
    response_format: S.optionalWith<typeof AssistantsApiResponseFormatOption, {
        nullable: true;
    }>;
}, S.Struct.Encoded<{
    model: S.Union<[typeof S.String, typeof AssistantSupportedModels]>;
    name: S.optionalWith<S.filter<typeof S.String>, {
        nullable: true;
    }>;
    description: S.optionalWith<S.filter<typeof S.String>, {
        nullable: true;
    }>;
    instructions: S.optionalWith<S.filter<typeof S.String>, {
        nullable: true;
    }>;
    reasoning_effort: S.optionalWith<typeof ReasoningEffort, {
        nullable: true;
        default: () => "medium";
    }>;
    tools: S.optionalWith<S.filter<S.Array$<S.Union<[typeof AssistantToolsCode, typeof AssistantToolsFileSearch, typeof AssistantToolsFunction]>>>, {
        nullable: true;
        default: () => readonly [];
    }>;
    tool_resources: S.optionalWith<S.Struct<{
        code_interpreter: S.optionalWith<S.Struct<{
            file_ids: S.optionalWith<S.filter<S.Array$<typeof S.String>>, {
                nullable: true;
                default: () => readonly [];
            }>;
        }>, {
            nullable: true;
        }>;
        file_search: S.optionalWith<S.Struct<{
            vector_store_ids: S.optionalWith<S.filter<S.Array$<typeof S.String>>, {
                nullable: true;
            }>;
            vector_stores: S.optionalWith<S.filter<S.Array$<S.Struct<{
                file_ids: S.optionalWith<S.filter<S.Array$<typeof S.String>>, {
                    nullable: true;
                }>;
                chunking_strategy: S.optionalWith<S.Record$<typeof S.String, typeof S.Unknown>, {
                    nullable: true;
                }>;
                metadata: S.optionalWith<typeof Metadata, {
                    nullable: true;
                }>;
            }>>>, {
                nullable: true;
            }>;
        }>, {
            nullable: true;
        }>;
    }>, {
        nullable: true;
    }>;
    metadata: S.optionalWith<typeof Metadata, {
        nullable: true;
    }>;
    temperature: S.optionalWith<S.filter<S.filter<typeof S.Number>>, {
        nullable: true;
        default: () => 1;
    }>;
    top_p: S.optionalWith<S.filter<S.filter<typeof S.Number>>, {
        nullable: true;
        default: () => 1;
    }>;
    response_format: S.optionalWith<typeof AssistantsApiResponseFormatOption, {
        nullable: true;
    }>;
}>, never, {
    readonly model: string;
} & {
    readonly description?: string | undefined;
} & {
    readonly name?: string | undefined;
} & {
    readonly instructions?: string | undefined;
} & {
    readonly tools?: readonly ({
        readonly type: "code_interpreter";
    } | {
        readonly type: "file_search";
        readonly file_search?: {
            readonly max_num_results?: number | undefined;
            readonly ranking_options?: {
                readonly ranker?: "auto" | "default_2024_08_21" | undefined;
                readonly score_threshold: number;
            } | undefined;
        } | undefined;
    } | {
        readonly function: {
            readonly description?: string | undefined;
            readonly name: string;
            readonly parameters?: {
                readonly [x: string]: unknown;
            } | undefined;
            readonly strict: boolean;
        };
        readonly type: "function";
    })[];
} & {
    readonly tool_resources?: {
        readonly code_interpreter?: {
            readonly file_ids: readonly string[];
        } | undefined;
        readonly file_search?: {
            readonly vector_store_ids?: readonly string[] | undefined;
            readonly vector_stores?: readonly {
                readonly metadata?: {
                    readonly [x: string]: unknown;
                } | undefined;
                readonly file_ids?: readonly string[] | undefined;
                readonly chunking_strategy?: {
                    readonly [x: string]: unknown;
                } | undefined;
            }[] | undefined;
        } | undefined;
    } | undefined;
} & {
    readonly metadata?: {
        readonly [x: string]: unknown;
    } | undefined;
} & {
    readonly temperature?: number;
} & {
    readonly top_p?: number;
} & {
    readonly response_format?: "auto" | {
        readonly type: "text";
    } | {
        readonly type: "json_object";
    } | {
        readonly type: "json_schema";
        readonly json_schema: {
            readonly description?: string | undefined;
            readonly name: string;
            readonly strict: boolean;
            readonly schema?: {
                readonly [x: string]: unknown;
            } | undefined;
        };
    } | undefined;
} & {
    readonly reasoning_effort?: "low" | "medium" | "high";
}, {}, {}>;
export declare class CreateAssistantRequest extends CreateAssistantRequest_base {
}
declare const ModifyAssistantRequest_base: S.Class<ModifyAssistantRequest, {
    model: S.optionalWith<S.Union<[typeof S.String, typeof AssistantSupportedModels]>, {
        nullable: true;
    }>;
    reasoning_effort: S.optionalWith<typeof ReasoningEffort, {
        nullable: true;
        default: () => "medium";
    }>;
    name: S.optionalWith<S.filter<typeof S.String>, {
        nullable: true;
    }>;
    description: S.optionalWith<S.filter<typeof S.String>, {
        nullable: true;
    }>;
    instructions: S.optionalWith<S.filter<typeof S.String>, {
        nullable: true;
    }>;
    tools: S.optionalWith<S.filter<S.Array$<S.Union<[typeof AssistantToolsCode, typeof AssistantToolsFileSearch, typeof AssistantToolsFunction]>>>, {
        nullable: true;
        default: () => readonly [];
    }>;
    tool_resources: S.optionalWith<S.Struct<{
        code_interpreter: S.optionalWith<S.Struct<{
            file_ids: S.optionalWith<S.filter<S.Array$<typeof S.String>>, {
                nullable: true;
                default: () => readonly [];
            }>;
        }>, {
            nullable: true;
        }>;
        file_search: S.optionalWith<S.Struct<{
            vector_store_ids: S.optionalWith<S.filter<S.Array$<typeof S.String>>, {
                nullable: true;
            }>;
        }>, {
            nullable: true;
        }>;
    }>, {
        nullable: true;
    }>;
    metadata: S.optionalWith<typeof Metadata, {
        nullable: true;
    }>;
    temperature: S.optionalWith<S.filter<S.filter<typeof S.Number>>, {
        nullable: true;
        default: () => 1;
    }>;
    top_p: S.optionalWith<S.filter<S.filter<typeof S.Number>>, {
        nullable: true;
        default: () => 1;
    }>;
    response_format: S.optionalWith<typeof AssistantsApiResponseFormatOption, {
        nullable: true;
    }>;
}, S.Struct.Encoded<{
    model: S.optionalWith<S.Union<[typeof S.String, typeof AssistantSupportedModels]>, {
        nullable: true;
    }>;
    reasoning_effort: S.optionalWith<typeof ReasoningEffort, {
        nullable: true;
        default: () => "medium";
    }>;
    name: S.optionalWith<S.filter<typeof S.String>, {
        nullable: true;
    }>;
    description: S.optionalWith<S.filter<typeof S.String>, {
        nullable: true;
    }>;
    instructions: S.optionalWith<S.filter<typeof S.String>, {
        nullable: true;
    }>;
    tools: S.optionalWith<S.filter<S.Array$<S.Union<[typeof AssistantToolsCode, typeof AssistantToolsFileSearch, typeof AssistantToolsFunction]>>>, {
        nullable: true;
        default: () => readonly [];
    }>;
    tool_resources: S.optionalWith<S.Struct<{
        code_interpreter: S.optionalWith<S.Struct<{
            file_ids: S.optionalWith<S.filter<S.Array$<typeof S.String>>, {
                nullable: true;
                default: () => readonly [];
            }>;
        }>, {
            nullable: true;
        }>;
        file_search: S.optionalWith<S.Struct<{
            vector_store_ids: S.optionalWith<S.filter<S.Array$<typeof S.String>>, {
                nullable: true;
            }>;
        }>, {
            nullable: true;
        }>;
    }>, {
        nullable: true;
    }>;
    metadata: S.optionalWith<typeof Metadata, {
        nullable: true;
    }>;
    temperature: S.optionalWith<S.filter<S.filter<typeof S.Number>>, {
        nullable: true;
        default: () => 1;
    }>;
    top_p: S.optionalWith<S.filter<S.filter<typeof S.Number>>, {
        nullable: true;
        default: () => 1;
    }>;
    response_format: S.optionalWith<typeof AssistantsApiResponseFormatOption, {
        nullable: true;
    }>;
}>, never, {
    readonly model?: string | undefined;
} & {
    readonly description?: string | undefined;
} & {
    readonly name?: string | undefined;
} & {
    readonly instructions?: string | undefined;
} & {
    readonly tools?: readonly ({
        readonly type: "code_interpreter";
    } | {
        readonly type: "file_search";
        readonly file_search?: {
            readonly max_num_results?: number | undefined;
            readonly ranking_options?: {
                readonly ranker?: "auto" | "default_2024_08_21" | undefined;
                readonly score_threshold: number;
            } | undefined;
        } | undefined;
    } | {
        readonly function: {
            readonly description?: string | undefined;
            readonly name: string;
            readonly parameters?: {
                readonly [x: string]: unknown;
            } | undefined;
            readonly strict: boolean;
        };
        readonly type: "function";
    })[];
} & {
    readonly tool_resources?: {
        readonly code_interpreter?: {
            readonly file_ids: readonly string[];
        } | undefined;
        readonly file_search?: {
            readonly vector_store_ids?: readonly string[] | undefined;
        } | undefined;
    } | undefined;
} & {
    readonly metadata?: {
        readonly [x: string]: unknown;
    } | undefined;
} & {
    readonly temperature?: number;
} & {
    readonly top_p?: number;
} & {
    readonly response_format?: "auto" | {
        readonly type: "text";
    } | {
        readonly type: "json_object";
    } | {
        readonly type: "json_schema";
        readonly json_schema: {
            readonly description?: string | undefined;
            readonly name: string;
            readonly strict: boolean;
            readonly schema?: {
                readonly [x: string]: unknown;
            } | undefined;
        };
    } | undefined;
} & {
    readonly reasoning_effort?: "low" | "medium" | "high";
}, {}, {}>;
export declare class ModifyAssistantRequest extends ModifyAssistantRequest_base {
}
declare const DeleteAssistantResponseObject_base: S.Literal<["assistant.deleted"]>;
export declare class DeleteAssistantResponseObject extends DeleteAssistantResponseObject_base {
}
declare const DeleteAssistantResponse_base: S.Class<DeleteAssistantResponse, {
    id: typeof S.String;
    deleted: typeof S.Boolean;
    object: typeof DeleteAssistantResponseObject;
}, S.Struct.Encoded<{
    id: typeof S.String;
    deleted: typeof S.Boolean;
    object: typeof DeleteAssistantResponseObject;
}>, never, {
    readonly object: "assistant.deleted";
} & {
    readonly id: string;
} & {
    readonly deleted: boolean;
}, {}, {}>;
export declare class DeleteAssistantResponse extends DeleteAssistantResponse_base {
}
declare const CreateSpeechRequestModelEnum_base: S.Literal<["tts-1", "tts-1-hd", "gpt-4o-mini-tts"]>;
export declare class CreateSpeechRequestModelEnum extends CreateSpeechRequestModelEnum_base {
}
declare const VoiceIdsSharedEnum_base: S.Literal<["alloy", "ash", "ballad", "coral", "echo", "fable", "onyx", "nova", "sage", "shimmer", "verse"]>;
export declare class VoiceIdsSharedEnum extends VoiceIdsSharedEnum_base {
}
declare const VoiceIdsShared_base: S.Union<[typeof S.String, typeof VoiceIdsSharedEnum]>;
export declare class VoiceIdsShared extends VoiceIdsShared_base {
}
declare const CreateSpeechRequestResponseFormat_base: S.Literal<["mp3", "opus", "aac", "flac", "wav", "pcm"]>;
export declare class CreateSpeechRequestResponseFormat extends CreateSpeechRequestResponseFormat_base {
}
declare const CreateSpeechRequest_base: S.Class<CreateSpeechRequest, {
    model: S.Union<[typeof S.String, typeof CreateSpeechRequestModelEnum]>;
    input: S.filter<typeof S.String>;
    instructions: S.optionalWith<S.filter<typeof S.String>, {
        nullable: true;
    }>;
    voice: typeof VoiceIdsShared;
    response_format: S.optionalWith<typeof CreateSpeechRequestResponseFormat, {
        nullable: true;
        default: () => "mp3";
    }>;
    speed: S.optionalWith<S.filter<S.filter<typeof S.Number>>, {
        nullable: true;
        default: () => 1;
    }>;
}, S.Struct.Encoded<{
    model: S.Union<[typeof S.String, typeof CreateSpeechRequestModelEnum]>;
    input: S.filter<typeof S.String>;
    instructions: S.optionalWith<S.filter<typeof S.String>, {
        nullable: true;
    }>;
    voice: typeof VoiceIdsShared;
    response_format: S.optionalWith<typeof CreateSpeechRequestResponseFormat, {
        nullable: true;
        default: () => "mp3";
    }>;
    speed: S.optionalWith<S.filter<S.filter<typeof S.Number>>, {
        nullable: true;
        default: () => 1;
    }>;
}>, never, {
    readonly model: string;
} & {
    readonly instructions?: string | undefined;
} & {
    readonly response_format?: "mp3" | "opus" | "aac" | "flac" | "wav" | "pcm";
} & {
    readonly voice: string;
} & {
    readonly input: string;
} & {
    readonly speed?: number;
}, {}, {}>;
export declare class CreateSpeechRequest extends CreateSpeechRequest_base {
}
declare const LogProbProperties_base: S.Struct<{
    token: typeof S.String;
    logprob: typeof S.Number;
    bytes: S.Array$<typeof S.Int>;
}>;
export declare class LogProbProperties extends LogProbProperties_base {
}
declare const CreateTranscriptionResponseJson_base: S.Struct<{
    text: typeof S.String;
    logprobs: S.optionalWith<S.Array$<typeof LogProbProperties>, {
        nullable: true;
    }>;
}>;
export declare class CreateTranscriptionResponseJson extends CreateTranscriptionResponseJson_base {
}
declare const TranscriptionWord_base: S.Struct<{
    word: typeof S.String;
    start: typeof S.Number;
    end: typeof S.Number;
}>;
export declare class TranscriptionWord extends TranscriptionWord_base {
}
declare const TranscriptionSegment_base: S.Struct<{
    id: typeof S.Int;
    seek: typeof S.Int;
    start: typeof S.Number;
    end: typeof S.Number;
    text: typeof S.String;
    tokens: S.Array$<typeof S.Int>;
    temperature: typeof S.Number;
    avg_logprob: typeof S.Number;
    compression_ratio: typeof S.Number;
    no_speech_prob: typeof S.Number;
}>;
export declare class TranscriptionSegment extends TranscriptionSegment_base {
}
declare const CreateTranscriptionResponseVerboseJson_base: S.Struct<{
    language: typeof S.String;
    duration: typeof S.Number;
    text: typeof S.String;
    words: S.optionalWith<S.Array$<typeof TranscriptionWord>, {
        nullable: true;
    }>;
    segments: S.optionalWith<S.Array$<typeof TranscriptionSegment>, {
        nullable: true;
    }>;
}>;
export declare class CreateTranscriptionResponseVerboseJson extends CreateTranscriptionResponseVerboseJson_base {
}
declare const CreateTranscription200_base: S.Union<[typeof CreateTranscriptionResponseJson, typeof CreateTranscriptionResponseVerboseJson]>;
export declare class CreateTranscription200 extends CreateTranscription200_base {
}
declare const CreateTranslationResponseJson_base: S.Struct<{
    text: typeof S.String;
}>;
export declare class CreateTranslationResponseJson extends CreateTranslationResponseJson_base {
}
declare const CreateTranslationResponseVerboseJson_base: S.Struct<{
    language: typeof S.String;
    duration: typeof S.Number;
    text: typeof S.String;
    segments: S.optionalWith<S.Array$<typeof TranscriptionSegment>, {
        nullable: true;
    }>;
}>;
export declare class CreateTranslationResponseVerboseJson extends CreateTranslationResponseVerboseJson_base {
}
declare const CreateTranslation200_base: S.Union<[typeof CreateTranslationResponseJson, typeof CreateTranslationResponseVerboseJson]>;
export declare class CreateTranslation200 extends CreateTranslation200_base {
}
declare const ListBatchesParams_base: S.Struct<{
    after: S.optionalWith<typeof S.String, {
        nullable: true;
    }>;
    limit: S.optionalWith<typeof S.Int, {
        nullable: true;
        default: () => 20;
    }>;
}>;
export declare class ListBatchesParams extends ListBatchesParams_base {
}
declare const BatchObject_base: S.Literal<["batch"]>;
export declare class BatchObject extends BatchObject_base {
}
declare const BatchStatus_base: S.Literal<["validating", "failed", "in_progress", "finalizing", "completed", "expired", "cancelling", "cancelled"]>;
export declare class BatchStatus extends BatchStatus_base {
}
declare const Batch_base: S.Struct<{
    id: typeof S.String;
    object: typeof BatchObject;
    endpoint: typeof S.String;
    errors: S.optionalWith<S.Struct<{
        object: S.optionalWith<typeof S.String, {
            nullable: true;
        }>;
        data: S.optionalWith<S.Array$<S.Struct<{
            code: S.optionalWith<typeof S.String, {
                nullable: true;
            }>;
            message: S.optionalWith<typeof S.String, {
                nullable: true;
            }>;
            param: S.optionalWith<typeof S.String, {
                nullable: true;
            }>;
            line: S.optionalWith<typeof S.Int, {
                nullable: true;
            }>;
        }>>, {
            nullable: true;
        }>;
    }>, {
        nullable: true;
    }>;
    input_file_id: typeof S.String;
    completion_window: typeof S.String;
    status: typeof BatchStatus;
    output_file_id: S.optionalWith<typeof S.String, {
        nullable: true;
    }>;
    error_file_id: S.optionalWith<typeof S.String, {
        nullable: true;
    }>;
    created_at: typeof S.Int;
    in_progress_at: S.optionalWith<typeof S.Int, {
        nullable: true;
    }>;
    expires_at: S.optionalWith<typeof S.Int, {
        nullable: true;
    }>;
    finalizing_at: S.optionalWith<typeof S.Int, {
        nullable: true;
    }>;
    completed_at: S.optionalWith<typeof S.Int, {
        nullable: true;
    }>;
    failed_at: S.optionalWith<typeof S.Int, {
        nullable: true;
    }>;
    expired_at: S.optionalWith<typeof S.Int, {
        nullable: true;
    }>;
    cancelling_at: S.optionalWith<typeof S.Int, {
        nullable: true;
    }>;
    cancelled_at: S.optionalWith<typeof S.Int, {
        nullable: true;
    }>;
    request_counts: S.optionalWith<S.Struct<{
        total: typeof S.Int;
        completed: typeof S.Int;
        failed: typeof S.Int;
    }>, {
        nullable: true;
    }>;
    metadata: S.optionalWith<typeof Metadata, {
        nullable: true;
    }>;
}>;
export declare class Batch extends Batch_base {
}
declare const ListBatchesResponseObject_base: S.Literal<["list"]>;
export declare class ListBatchesResponseObject extends ListBatchesResponseObject_base {
}
declare const ListBatchesResponse_base: S.Class<ListBatchesResponse, {
    data: S.Array$<typeof Batch>;
    first_id: S.optionalWith<typeof S.String, {
        nullable: true;
    }>;
    last_id: S.optionalWith<typeof S.String, {
        nullable: true;
    }>;
    has_more: typeof S.Boolean;
    object: typeof ListBatchesResponseObject;
}, S.Struct.Encoded<{
    data: S.Array$<typeof Batch>;
    first_id: S.optionalWith<typeof S.String, {
        nullable: true;
    }>;
    last_id: S.optionalWith<typeof S.String, {
        nullable: true;
    }>;
    has_more: typeof S.Boolean;
    object: typeof ListBatchesResponseObject;
}>, never, {
    readonly object: "list";
} & {
    readonly first_id?: string | undefined;
} & {
    readonly last_id?: string | undefined;
} & {
    readonly has_more: boolean;
} & {
    readonly data: readonly {
        readonly object: "batch";
        readonly id: string;
        readonly created_at: number;
        readonly metadata?: {
            readonly [x: string]: unknown;
        } | undefined;
        readonly endpoint: string;
        readonly errors?: {
            readonly object?: string | undefined;
            readonly data?: readonly {
                readonly message?: string | undefined;
                readonly code?: string | undefined;
                readonly param?: string | undefined;
                readonly line?: number | undefined;
            }[] | undefined;
        } | undefined;
        readonly input_file_id: string;
        readonly completion_window: string;
        readonly status: "validating" | "failed" | "in_progress" | "finalizing" | "completed" | "expired" | "cancelling" | "cancelled";
        readonly output_file_id?: string | undefined;
        readonly error_file_id?: string | undefined;
        readonly in_progress_at?: number | undefined;
        readonly expires_at?: number | undefined;
        readonly finalizing_at?: number | undefined;
        readonly completed_at?: number | undefined;
        readonly failed_at?: number | undefined;
        readonly expired_at?: number | undefined;
        readonly cancelling_at?: number | undefined;
        readonly cancelled_at?: number | undefined;
        readonly request_counts?: {
            readonly failed: number;
            readonly completed: number;
            readonly total: number;
        } | undefined;
    }[];
}, {}, {}>;
export declare class ListBatchesResponse extends ListBatchesResponse_base {
}
declare const CreateBatchRequestEndpoint_base: S.Literal<["/v1/responses", "/v1/chat/completions", "/v1/embeddings", "/v1/completions"]>;
export declare class CreateBatchRequestEndpoint extends CreateBatchRequestEndpoint_base {
}
declare const CreateBatchRequestCompletionWindow_base: S.Literal<["24h"]>;
export declare class CreateBatchRequestCompletionWindow extends CreateBatchRequestCompletionWindow_base {
}
declare const CreateBatchRequest_base: S.Class<CreateBatchRequest, {
    input_file_id: typeof S.String;
    endpoint: typeof CreateBatchRequestEndpoint;
    completion_window: typeof CreateBatchRequestCompletionWindow;
    metadata: S.optionalWith<typeof Metadata, {
        nullable: true;
    }>;
}, S.Struct.Encoded<{
    input_file_id: typeof S.String;
    endpoint: typeof CreateBatchRequestEndpoint;
    completion_window: typeof CreateBatchRequestCompletionWindow;
    metadata: S.optionalWith<typeof Metadata, {
        nullable: true;
    }>;
}>, never, {
    readonly metadata?: {
        readonly [x: string]: unknown;
    } | undefined;
} & {
    readonly endpoint: "/v1/responses" | "/v1/chat/completions" | "/v1/embeddings" | "/v1/completions";
} & {
    readonly input_file_id: string;
} & {
    readonly completion_window: "24h";
}, {}, {}>;
export declare class CreateBatchRequest extends CreateBatchRequest_base {
}
declare const ListChatCompletionsParamsOrder_base: S.Literal<["asc", "desc"]>;
export declare class ListChatCompletionsParamsOrder extends ListChatCompletionsParamsOrder_base {
}
declare const ListChatCompletionsParams_base: S.Struct<{
    model: S.optionalWith<typeof S.String, {
        nullable: true;
    }>;
    metadata: S.optionalWith<typeof Metadata, {
        nullable: true;
    }>;
    after: S.optionalWith<typeof S.String, {
        nullable: true;
    }>;
    limit: S.optionalWith<typeof S.Int, {
        nullable: true;
        default: () => 20;
    }>;
    order: S.optionalWith<typeof ListChatCompletionsParamsOrder, {
        nullable: true;
        default: () => "asc";
    }>;
}>;
export declare class ListChatCompletionsParams extends ListChatCompletionsParams_base {
}
declare const ChatCompletionListObject_base: S.Literal<["list"]>;
export declare class ChatCompletionListObject extends ChatCompletionListObject_base {
}
declare const ChatCompletionMessageToolCallType_base: S.Literal<["function"]>;
export declare class ChatCompletionMessageToolCallType extends ChatCompletionMessageToolCallType_base {
}
declare const ChatCompletionMessageToolCall_base: S.Struct<{
    id: typeof S.String;
    type: typeof ChatCompletionMessageToolCallType;
    function: S.Struct<{
        name: typeof S.String;
        arguments: typeof S.String;
    }>;
}>;
export declare class ChatCompletionMessageToolCall extends ChatCompletionMessageToolCall_base {
}
declare const ChatCompletionMessageToolCalls_base: S.Array$<typeof ChatCompletionMessageToolCall>;
export declare class ChatCompletionMessageToolCalls extends ChatCompletionMessageToolCalls_base {
}
declare const ChatCompletionResponseMessageRole_base: S.Literal<["assistant"]>;
export declare class ChatCompletionResponseMessageRole extends ChatCompletionResponseMessageRole_base {
}
declare const ChatCompletionResponseMessage_base: S.Struct<{
    content: S.NullOr<typeof S.String>;
    refusal: S.NullOr<typeof S.String>;
    tool_calls: S.optionalWith<typeof ChatCompletionMessageToolCalls, {
        nullable: true;
    }>;
    annotations: S.optionalWith<S.Array$<S.Struct<{
        type: S.Literal<["url_citation"]>;
        url_citation: S.Struct<{
            end_index: typeof S.Int;
            start_index: typeof S.Int;
            url: typeof S.String;
            title: typeof S.String;
        }>;
    }>>, {
        nullable: true;
    }>;
    role: typeof ChatCompletionResponseMessageRole;
    function_call: S.optionalWith<S.Struct<{
        arguments: typeof S.String;
        name: typeof S.String;
    }>, {
        nullable: true;
    }>;
    audio: S.optionalWith<S.Struct<{
        id: typeof S.String;
        expires_at: typeof S.Int;
        data: typeof S.String;
        transcript: typeof S.String;
    }>, {
        nullable: true;
    }>;
}>;
export declare class ChatCompletionResponseMessage extends ChatCompletionResponseMessage_base {
}
declare const ChatCompletionTokenLogprob_base: S.Struct<{
    token: typeof S.String;
    logprob: typeof S.Number;
    bytes: S.NullOr<S.Array$<typeof S.Int>>;
    top_logprobs: S.Array$<S.Struct<{
        token: typeof S.String;
        logprob: typeof S.Number;
        bytes: S.NullOr<S.Array$<typeof S.Int>>;
    }>>;
}>;
export declare class ChatCompletionTokenLogprob extends ChatCompletionTokenLogprob_base {
}
declare const CreateChatCompletionResponseServiceTier_base: S.Literal<["scale", "default"]>;
export declare class CreateChatCompletionResponseServiceTier extends CreateChatCompletionResponseServiceTier_base {
}
declare const CreateChatCompletionResponseObject_base: S.Literal<["chat.completion"]>;
export declare class CreateChatCompletionResponseObject extends CreateChatCompletionResponseObject_base {
}
declare const CompletionUsage_base: S.Struct<{
    completion_tokens: S.PropertySignature<":", number, never, ":", number, true, never>;
    prompt_tokens: S.PropertySignature<":", number, never, ":", number, true, never>;
    total_tokens: S.PropertySignature<":", number, never, ":", number, true, never>;
    completion_tokens_details: S.optionalWith<S.Struct<{
        accepted_prediction_tokens: S.optionalWith<typeof S.Int, {
            nullable: true;
            default: () => 0;
        }>;
        audio_tokens: S.optionalWith<typeof S.Int, {
            nullable: true;
            default: () => 0;
        }>;
        reasoning_tokens: S.optionalWith<typeof S.Int, {
            nullable: true;
            default: () => 0;
        }>;
        rejected_prediction_tokens: S.optionalWith<typeof S.Int, {
            nullable: true;
            default: () => 0;
        }>;
    }>, {
        nullable: true;
    }>;
    prompt_tokens_details: S.optionalWith<S.Struct<{
        audio_tokens: S.optionalWith<typeof S.Int, {
            nullable: true;
            default: () => 0;
        }>;
        cached_tokens: S.optionalWith<typeof S.Int, {
            nullable: true;
            default: () => 0;
        }>;
    }>, {
        nullable: true;
    }>;
}>;
export declare class CompletionUsage extends CompletionUsage_base {
}
declare const CreateChatCompletionResponse_base: S.Struct<{
    id: typeof S.String;
    choices: S.Array$<S.Struct<{
        finish_reason: S.Literal<["stop", "length", "tool_calls", "content_filter", "function_call"]>;
        index: typeof S.Int;
        message: typeof ChatCompletionResponseMessage;
        logprobs: S.optionalWith<S.Struct<{
            content: S.NullOr<S.Array$<typeof ChatCompletionTokenLogprob>>;
            refusal: S.NullOr<S.Array$<typeof ChatCompletionTokenLogprob>>;
        }>, {
            nullable: true;
        }>;
    }>>;
    created: typeof S.Int;
    model: typeof S.String;
    service_tier: S.optionalWith<typeof CreateChatCompletionResponseServiceTier, {
        nullable: true;
    }>;
    system_fingerprint: S.optionalWith<typeof S.String, {
        nullable: true;
    }>;
    object: typeof CreateChatCompletionResponseObject;
    usage: S.optionalWith<typeof CompletionUsage, {
        nullable: true;
    }>;
}>;
export declare class CreateChatCompletionResponse extends CreateChatCompletionResponse_base {
}
declare const ChatCompletionList_base: S.Class<ChatCompletionList, {
    object: S.PropertySignature<":", "list", never, ":", "list", true, never>;
    data: S.Array$<typeof CreateChatCompletionResponse>;
    first_id: typeof S.String;
    last_id: typeof S.String;
    has_more: typeof S.Boolean;
}, S.Struct.Encoded<{
    object: S.PropertySignature<":", "list", never, ":", "list", true, never>;
    data: S.Array$<typeof CreateChatCompletionResponse>;
    first_id: typeof S.String;
    last_id: typeof S.String;
    has_more: typeof S.Boolean;
}>, never, {
    readonly object?: "list";
} & {
    readonly first_id: string;
} & {
    readonly last_id: string;
} & {
    readonly has_more: boolean;
} & {
    readonly data: readonly {
        readonly object: "chat.completion";
        readonly model: string;
        readonly id: string;
        readonly choices: readonly {
            readonly message: {
                readonly annotations?: readonly {
                    readonly type: "url_citation";
                    readonly url_citation: {
                        readonly end_index: number;
                        readonly start_index: number;
                        readonly url: string;
                        readonly title: string;
                    };
                }[] | undefined;
                readonly content: string | null;
                readonly role: "assistant";
                readonly refusal: string | null;
                readonly tool_calls?: readonly {
                    readonly function: {
                        readonly name: string;
                        readonly arguments: string;
                    };
                    readonly type: "function";
                    readonly id: string;
                }[] | undefined;
                readonly function_call?: {
                    readonly name: string;
                    readonly arguments: string;
                } | undefined;
                readonly audio?: {
                    readonly id: string;
                    readonly data: string;
                    readonly expires_at: number;
                    readonly transcript: string;
                } | undefined;
            };
            readonly logprobs?: {
                readonly content: readonly {
                    readonly token: string;
                    readonly logprob: number;
                    readonly bytes: readonly number[] | null;
                    readonly top_logprobs: readonly {
                        readonly token: string;
                        readonly logprob: number;
                        readonly bytes: readonly number[] | null;
                    }[];
                }[] | null;
                readonly refusal: readonly {
                    readonly token: string;
                    readonly logprob: number;
                    readonly bytes: readonly number[] | null;
                    readonly top_logprobs: readonly {
                        readonly token: string;
                        readonly logprob: number;
                        readonly bytes: readonly number[] | null;
                    }[];
                }[] | null;
            } | undefined;
            readonly finish_reason: "length" | "tool_calls" | "function_call" | "stop" | "content_filter";
            readonly index: number;
        }[];
        readonly created: number;
        readonly service_tier?: "default" | "scale" | undefined;
        readonly system_fingerprint?: string | undefined;
        readonly usage?: {
            readonly completion_tokens: number;
            readonly prompt_tokens: number;
            readonly total_tokens: number;
            readonly completion_tokens_details?: {
                readonly accepted_prediction_tokens: number;
                readonly audio_tokens: number;
                readonly reasoning_tokens: number;
                readonly rejected_prediction_tokens: number;
            } | undefined;
            readonly prompt_tokens_details?: {
                readonly audio_tokens: number;
                readonly cached_tokens: number;
            } | undefined;
        } | undefined;
    }[];
}, {}, {}>;
export declare class ChatCompletionList extends ChatCompletionList_base {
}
declare const ChatCompletionRequestMessageContentPartTextType_base: S.Literal<["text"]>;
export declare class ChatCompletionRequestMessageContentPartTextType extends ChatCompletionRequestMessageContentPartTextType_base {
}
declare const ChatCompletionRequestMessageContentPartText_base: S.Struct<{
    type: typeof ChatCompletionRequestMessageContentPartTextType;
    text: typeof S.String;
}>;
export declare class ChatCompletionRequestMessageContentPartText extends ChatCompletionRequestMessageContentPartText_base {
}
declare const ChatCompletionRequestDeveloperMessageRole_base: S.Literal<["developer"]>;
export declare class ChatCompletionRequestDeveloperMessageRole extends ChatCompletionRequestDeveloperMessageRole_base {
}
declare const ChatCompletionRequestDeveloperMessage_base: S.Struct<{
    content: S.Union<[typeof S.String, S.NonEmptyArray<typeof ChatCompletionRequestMessageContentPartText>]>;
    role: typeof ChatCompletionRequestDeveloperMessageRole;
    name: S.optionalWith<typeof S.String, {
        nullable: true;
    }>;
}>;
export declare class ChatCompletionRequestDeveloperMessage extends ChatCompletionRequestDeveloperMessage_base {
}
export declare class ChatCompletionRequestSystemMessageContentPart extends ChatCompletionRequestMessageContentPartText {
}
declare const ChatCompletionRequestSystemMessageRole_base: S.Literal<["system"]>;
export declare class ChatCompletionRequestSystemMessageRole extends ChatCompletionRequestSystemMessageRole_base {
}
declare const ChatCompletionRequestSystemMessage_base: S.Struct<{
    content: S.Union<[typeof S.String, S.NonEmptyArray<typeof ChatCompletionRequestSystemMessageContentPart>]>;
    role: typeof ChatCompletionRequestSystemMessageRole;
    name: S.optionalWith<typeof S.String, {
        nullable: true;
    }>;
}>;
export declare class ChatCompletionRequestSystemMessage extends ChatCompletionRequestSystemMessage_base {
}
declare const ChatCompletionRequestMessageContentPartImageType_base: S.Literal<["image_url"]>;
export declare class ChatCompletionRequestMessageContentPartImageType extends ChatCompletionRequestMessageContentPartImageType_base {
}
declare const ChatCompletionRequestMessageContentPartImageImageUrlDetail_base: S.Literal<["auto", "low", "high"]>;
export declare class ChatCompletionRequestMessageContentPartImageImageUrlDetail extends ChatCompletionRequestMessageContentPartImageImageUrlDetail_base {
}
declare const ChatCompletionRequestMessageContentPartImage_base: S.Struct<{
    type: typeof ChatCompletionRequestMessageContentPartImageType;
    image_url: S.Struct<{
        url: typeof S.String;
        detail: S.optionalWith<typeof ChatCompletionRequestMessageContentPartImageImageUrlDetail, {
            nullable: true;
            default: () => "auto";
        }>;
    }>;
}>;
export declare class ChatCompletionRequestMessageContentPartImage extends ChatCompletionRequestMessageContentPartImage_base {
}
declare const ChatCompletionRequestMessageContentPartAudioType_base: S.Literal<["input_audio"]>;
export declare class ChatCompletionRequestMessageContentPartAudioType extends ChatCompletionRequestMessageContentPartAudioType_base {
}
declare const ChatCompletionRequestMessageContentPartAudioInputAudioFormat_base: S.Literal<["wav", "mp3"]>;
export declare class ChatCompletionRequestMessageContentPartAudioInputAudioFormat extends ChatCompletionRequestMessageContentPartAudioInputAudioFormat_base {
}
declare const ChatCompletionRequestMessageContentPartAudio_base: S.Struct<{
    type: typeof ChatCompletionRequestMessageContentPartAudioType;
    input_audio: S.Struct<{
        data: typeof S.String;
        format: typeof ChatCompletionRequestMessageContentPartAudioInputAudioFormat;
    }>;
}>;
export declare class ChatCompletionRequestMessageContentPartAudio extends ChatCompletionRequestMessageContentPartAudio_base {
}
declare const ChatCompletionRequestMessageContentPartFileType_base: S.Literal<["file"]>;
export declare class ChatCompletionRequestMessageContentPartFileType extends ChatCompletionRequestMessageContentPartFileType_base {
}
declare const ChatCompletionRequestMessageContentPartFile_base: S.Struct<{
    type: typeof ChatCompletionRequestMessageContentPartFileType;
    file: S.Struct<{
        filename: S.optionalWith<typeof S.String, {
            nullable: true;
        }>;
        file_data: S.optionalWith<typeof S.String, {
            nullable: true;
        }>;
        file_id: S.optionalWith<typeof S.String, {
            nullable: true;
        }>;
    }>;
}>;
export declare class ChatCompletionRequestMessageContentPartFile extends ChatCompletionRequestMessageContentPartFile_base {
}
declare const ChatCompletionRequestUserMessageContentPart_base: S.Union<[typeof ChatCompletionRequestMessageContentPartText, typeof ChatCompletionRequestMessageContentPartImage, typeof ChatCompletionRequestMessageContentPartAudio, typeof ChatCompletionRequestMessageContentPartFile]>;
export declare class ChatCompletionRequestUserMessageContentPart extends ChatCompletionRequestUserMessageContentPart_base {
}
declare const ChatCompletionRequestUserMessageRole_base: S.Literal<["user"]>;
export declare class ChatCompletionRequestUserMessageRole extends ChatCompletionRequestUserMessageRole_base {
}
declare const ChatCompletionRequestUserMessage_base: S.Struct<{
    content: S.Union<[typeof S.String, S.NonEmptyArray<typeof ChatCompletionRequestUserMessageContentPart>]>;
    role: typeof ChatCompletionRequestUserMessageRole;
    name: S.optionalWith<typeof S.String, {
        nullable: true;
    }>;
}>;
export declare class ChatCompletionRequestUserMessage extends ChatCompletionRequestUserMessage_base {
}
declare const ChatCompletionRequestMessageContentPartRefusalType_base: S.Literal<["refusal"]>;
export declare class ChatCompletionRequestMessageContentPartRefusalType extends ChatCompletionRequestMessageContentPartRefusalType_base {
}
declare const ChatCompletionRequestMessageContentPartRefusal_base: S.Struct<{
    type: typeof ChatCompletionRequestMessageContentPartRefusalType;
    refusal: typeof S.String;
}>;
export declare class ChatCompletionRequestMessageContentPartRefusal extends ChatCompletionRequestMessageContentPartRefusal_base {
}
declare const ChatCompletionRequestAssistantMessageContentPart_base: S.Union<[typeof ChatCompletionRequestMessageContentPartText, typeof ChatCompletionRequestMessageContentPartRefusal]>;
export declare class ChatCompletionRequestAssistantMessageContentPart extends ChatCompletionRequestAssistantMessageContentPart_base {
}
declare const ChatCompletionRequestAssistantMessageRole_base: S.Literal<["assistant"]>;
export declare class ChatCompletionRequestAssistantMessageRole extends ChatCompletionRequestAssistantMessageRole_base {
}
declare const ChatCompletionRequestAssistantMessage_base: S.Struct<{
    content: S.optionalWith<S.Union<[typeof S.String, S.NonEmptyArray<typeof ChatCompletionRequestAssistantMessageContentPart>]>, {
        nullable: true;
    }>;
    refusal: S.optionalWith<typeof S.String, {
        nullable: true;
    }>;
    role: typeof ChatCompletionRequestAssistantMessageRole;
    name: S.optionalWith<typeof S.String, {
        nullable: true;
    }>;
    audio: S.optionalWith<S.Struct<{
        id: typeof S.String;
    }>, {
        nullable: true;
    }>;
    tool_calls: S.optionalWith<typeof ChatCompletionMessageToolCalls, {
        nullable: true;
    }>;
    function_call: S.optionalWith<S.Struct<{
        arguments: typeof S.String;
        name: typeof S.String;
    }>, {
        nullable: true;
    }>;
}>;
export declare class ChatCompletionRequestAssistantMessage extends ChatCompletionRequestAssistantMessage_base {
}
declare const ChatCompletionRequestToolMessageRole_base: S.Literal<["tool"]>;
export declare class ChatCompletionRequestToolMessageRole extends ChatCompletionRequestToolMessageRole_base {
}
export declare class ChatCompletionRequestToolMessageContentPart extends ChatCompletionRequestMessageContentPartText {
}
declare const ChatCompletionRequestToolMessage_base: S.Struct<{
    role: typeof ChatCompletionRequestToolMessageRole;
    content: S.Union<[typeof S.String, S.NonEmptyArray<typeof ChatCompletionRequestToolMessageContentPart>]>;
    tool_call_id: typeof S.String;
}>;
export declare class ChatCompletionRequestToolMessage extends ChatCompletionRequestToolMessage_base {
}
declare const ChatCompletionRequestFunctionMessageRole_base: S.Literal<["function"]>;
export declare class ChatCompletionRequestFunctionMessageRole extends ChatCompletionRequestFunctionMessageRole_base {
}
declare const ChatCompletionRequestFunctionMessage_base: S.Struct<{
    role: typeof ChatCompletionRequestFunctionMessageRole;
    content: S.NullOr<typeof S.String>;
    name: typeof S.String;
}>;
export declare class ChatCompletionRequestFunctionMessage extends ChatCompletionRequestFunctionMessage_base {
}
declare const ChatCompletionRequestMessage_base: S.Union<[typeof ChatCompletionRequestDeveloperMessage, typeof ChatCompletionRequestSystemMessage, typeof ChatCompletionRequestUserMessage, typeof ChatCompletionRequestAssistantMessage, typeof ChatCompletionRequestToolMessage, typeof ChatCompletionRequestFunctionMessage]>;
export declare class ChatCompletionRequestMessage extends ChatCompletionRequestMessage_base {
}
declare const ModelIdsSharedEnum_base: S.Literal<["o3-mini", "o3-mini-2025-01-31", "o1", "o1-2024-12-17", "o1-preview", "o1-preview-2024-09-12", "o1-mini", "o1-mini-2024-09-12", "gpt-4o", "gpt-4o-2024-11-20", "gpt-4o-2024-08-06", "gpt-4o-2024-05-13", "gpt-4o-audio-preview", "gpt-4o-audio-preview-2024-10-01", "gpt-4o-audio-preview-2024-12-17", "gpt-4o-mini-audio-preview", "gpt-4o-mini-audio-preview-2024-12-17", "gpt-4o-search-preview", "gpt-4o-mini-search-preview", "gpt-4o-search-preview-2025-03-11", "gpt-4o-mini-search-preview-2025-03-11", "chatgpt-4o-latest", "gpt-4o-mini", "gpt-4o-mini-2024-07-18", "gpt-4-turbo", "gpt-4-turbo-2024-04-09", "gpt-4-0125-preview", "gpt-4-turbo-preview", "gpt-4-1106-preview", "gpt-4-vision-preview", "gpt-4", "gpt-4-0314", "gpt-4-0613", "gpt-4-32k", "gpt-4-32k-0314", "gpt-4-32k-0613", "gpt-3.5-turbo", "gpt-3.5-turbo-16k", "gpt-3.5-turbo-0301", "gpt-3.5-turbo-0613", "gpt-3.5-turbo-1106", "gpt-3.5-turbo-0125", "gpt-3.5-turbo-16k-0613"]>;
export declare class ModelIdsSharedEnum extends ModelIdsSharedEnum_base {
}
declare const ModelIdsShared_base: S.Union<[typeof S.String, typeof ModelIdsSharedEnum]>;
export declare class ModelIdsShared extends ModelIdsShared_base {
}
declare const ResponseModalities_base: S.Array$<S.Literal<["text", "audio"]>>;
export declare class ResponseModalities extends ResponseModalities_base {
}
declare const CreateChatCompletionRequestWebSearchOptionsUserLocationType_base: S.Literal<["approximate"]>;
export declare class CreateChatCompletionRequestWebSearchOptionsUserLocationType extends CreateChatCompletionRequestWebSearchOptionsUserLocationType_base {
}
declare const WebSearchLocation_base: S.Struct<{
    country: S.optionalWith<typeof S.String, {
        nullable: true;
    }>;
    region: S.optionalWith<typeof S.String, {
        nullable: true;
    }>;
    city: S.optionalWith<typeof S.String, {
        nullable: true;
    }>;
    timezone: S.optionalWith<typeof S.String, {
        nullable: true;
    }>;
}>;
export declare class WebSearchLocation extends WebSearchLocation_base {
}
declare const WebSearchContextSize_base: S.Literal<["low", "medium", "high"]>;
export declare class WebSearchContextSize extends WebSearchContextSize_base {
}
declare const CreateChatCompletionRequestServiceTier_base: S.Literal<["auto", "default"]>;
export declare class CreateChatCompletionRequestServiceTier extends CreateChatCompletionRequestServiceTier_base {
}
declare const CreateChatCompletionRequestAudioFormat_base: S.Literal<["wav", "mp3", "flac", "opus", "pcm16"]>;
export declare class CreateChatCompletionRequestAudioFormat extends CreateChatCompletionRequestAudioFormat_base {
}
declare const StopConfiguration_base: S.Union<[typeof S.String, S.filter<S.filter<S.Array$<typeof S.String>>>]>;
export declare class StopConfiguration extends StopConfiguration_base {
}
declare const PredictionContentType_base: S.Literal<["content"]>;
export declare class PredictionContentType extends PredictionContentType_base {
}
declare const PredictionContent_base: S.Struct<{
    type: typeof PredictionContentType;
    content: S.Union<[typeof S.String, S.NonEmptyArray<typeof ChatCompletionRequestMessageContentPartText>]>;
}>;
export declare class PredictionContent extends PredictionContent_base {
}
declare const ChatCompletionStreamOptions_base: S.Struct<{
    include_usage: S.optionalWith<typeof S.Boolean, {
        nullable: true;
    }>;
}>;
export declare class ChatCompletionStreamOptions extends ChatCompletionStreamOptions_base {
}
declare const ChatCompletionToolType_base: S.Literal<["function"]>;
export declare class ChatCompletionToolType extends ChatCompletionToolType_base {
}
declare const ChatCompletionTool_base: S.Struct<{
    type: typeof ChatCompletionToolType;
    function: typeof FunctionObject;
}>;
export declare class ChatCompletionTool extends ChatCompletionTool_base {
}
declare const ChatCompletionToolChoiceOptionEnum_base: S.Literal<["none", "auto", "required"]>;
export declare class ChatCompletionToolChoiceOptionEnum extends ChatCompletionToolChoiceOptionEnum_base {
}
declare const ChatCompletionNamedToolChoiceType_base: S.Literal<["function"]>;
export declare class ChatCompletionNamedToolChoiceType extends ChatCompletionNamedToolChoiceType_base {
}
declare const ChatCompletionNamedToolChoice_base: S.Struct<{
    type: typeof ChatCompletionNamedToolChoiceType;
    function: S.Struct<{
        name: typeof S.String;
    }>;
}>;
export declare class ChatCompletionNamedToolChoice extends ChatCompletionNamedToolChoice_base {
}
declare const ChatCompletionToolChoiceOption_base: S.Union<[typeof ChatCompletionToolChoiceOptionEnum, typeof ChatCompletionNamedToolChoice]>;
export declare class ChatCompletionToolChoiceOption extends ChatCompletionToolChoiceOption_base {
}
export declare class ParallelToolCalls extends S.Boolean {
}
declare const CreateChatCompletionRequestFunctionCallEnum_base: S.Literal<["none", "auto"]>;
export declare class CreateChatCompletionRequestFunctionCallEnum extends CreateChatCompletionRequestFunctionCallEnum_base {
}
declare const ChatCompletionFunctionCallOption_base: S.Struct<{
    name: typeof S.String;
}>;
export declare class ChatCompletionFunctionCallOption extends ChatCompletionFunctionCallOption_base {
}
declare const ChatCompletionFunctions_base: S.Struct<{
    description: S.optionalWith<typeof S.String, {
        nullable: true;
    }>;
    name: typeof S.String;
    parameters: S.optionalWith<typeof FunctionParameters, {
        nullable: true;
    }>;
}>;
export declare class ChatCompletionFunctions extends ChatCompletionFunctions_base {
}
declare const CreateChatCompletionRequest_base: S.Class<CreateChatCompletionRequest, {
    messages: S.NonEmptyArray<typeof ChatCompletionRequestMessage>;
    model: typeof ModelIdsShared;
    modalities: S.optionalWith<typeof ResponseModalities, {
        nullable: true;
    }>;
    reasoning_effort: S.optionalWith<typeof ReasoningEffort, {
        nullable: true;
        default: () => "medium";
    }>;
    max_completion_tokens: S.optionalWith<typeof S.Int, {
        nullable: true;
    }>;
    frequency_penalty: S.optionalWith<S.filter<S.filter<typeof S.Number>>, {
        nullable: true;
        default: () => 0;
    }>;
    presence_penalty: S.optionalWith<S.filter<S.filter<typeof S.Number>>, {
        nullable: true;
        default: () => 0;
    }>;
    web_search_options: S.optionalWith<S.Struct<{
        user_location: S.optionalWith<S.Struct<{
            type: typeof CreateChatCompletionRequestWebSearchOptionsUserLocationType;
            approximate: typeof WebSearchLocation;
        }>, {
            nullable: true;
        }>;
        search_context_size: S.optionalWith<typeof WebSearchContextSize, {
            nullable: true;
            default: () => "medium";
        }>;
    }>, {
        nullable: true;
    }>;
    top_logprobs: S.optionalWith<S.filter<S.filter<typeof S.Int>>, {
        nullable: true;
    }>;
    response_format: S.optionalWith<S.Union<[typeof ResponseFormatText, typeof ResponseFormatJsonSchema, typeof ResponseFormatJsonObject]>, {
        nullable: true;
    }>;
    service_tier: S.optionalWith<typeof CreateChatCompletionRequestServiceTier, {
        nullable: true;
        default: () => "auto";
    }>;
    audio: S.optionalWith<S.Struct<{
        voice: typeof VoiceIdsShared;
        format: typeof CreateChatCompletionRequestAudioFormat;
    }>, {
        nullable: true;
    }>;
    store: S.optionalWith<typeof S.Boolean, {
        nullable: true;
        default: () => false;
    }>;
    stream: S.optionalWith<typeof S.Boolean, {
        nullable: true;
        default: () => false;
    }>;
    stop: S.optionalWith<S.NullOr<typeof StopConfiguration>, {
        default: () => null;
    }>;
    logit_bias: S.optionalWith<S.NullOr<S.Record$<typeof S.String, typeof S.Unknown>>, {
        default: () => null;
    }>;
    logprobs: S.optionalWith<typeof S.Boolean, {
        nullable: true;
        default: () => false;
    }>;
    max_tokens: S.optionalWith<typeof S.Int, {
        nullable: true;
    }>;
    n: S.optionalWith<S.filter<S.filter<typeof S.Int>>, {
        nullable: true;
        default: () => 1;
    }>;
    prediction: S.optionalWith<typeof PredictionContent, {
        nullable: true;
    }>;
    seed: S.optionalWith<typeof S.Int, {
        nullable: true;
    }>;
    stream_options: S.optionalWith<S.NullOr<typeof ChatCompletionStreamOptions>, {
        default: () => null;
    }>;
    tools: S.optionalWith<S.Array$<typeof ChatCompletionTool>, {
        nullable: true;
    }>;
    tool_choice: S.optionalWith<typeof ChatCompletionToolChoiceOption, {
        nullable: true;
    }>;
    parallel_tool_calls: S.optionalWith<typeof ParallelToolCalls, {
        nullable: true;
        default: () => true;
    }>;
    function_call: S.optionalWith<S.Union<[typeof CreateChatCompletionRequestFunctionCallEnum, typeof ChatCompletionFunctionCallOption]>, {
        nullable: true;
    }>;
    functions: S.optionalWith<S.filter<S.filter<S.Array$<typeof ChatCompletionFunctions>>>, {
        nullable: true;
    }>;
    metadata: S.optionalWith<typeof Metadata, {
        nullable: true;
    }>;
    temperature: S.optionalWith<S.filter<S.filter<typeof S.Number>>, {
        nullable: true;
        default: () => 1;
    }>;
    top_p: S.optionalWith<S.filter<S.filter<typeof S.Number>>, {
        nullable: true;
        default: () => 1;
    }>;
    user: S.optionalWith<typeof S.String, {
        nullable: true;
    }>;
}, S.Struct.Encoded<{
    messages: S.NonEmptyArray<typeof ChatCompletionRequestMessage>;
    model: typeof ModelIdsShared;
    modalities: S.optionalWith<typeof ResponseModalities, {
        nullable: true;
    }>;
    reasoning_effort: S.optionalWith<typeof ReasoningEffort, {
        nullable: true;
        default: () => "medium";
    }>;
    max_completion_tokens: S.optionalWith<typeof S.Int, {
        nullable: true;
    }>;
    frequency_penalty: S.optionalWith<S.filter<S.filter<typeof S.Number>>, {
        nullable: true;
        default: () => 0;
    }>;
    presence_penalty: S.optionalWith<S.filter<S.filter<typeof S.Number>>, {
        nullable: true;
        default: () => 0;
    }>;
    web_search_options: S.optionalWith<S.Struct<{
        user_location: S.optionalWith<S.Struct<{
            type: typeof CreateChatCompletionRequestWebSearchOptionsUserLocationType;
            approximate: typeof WebSearchLocation;
        }>, {
            nullable: true;
        }>;
        search_context_size: S.optionalWith<typeof WebSearchContextSize, {
            nullable: true;
            default: () => "medium";
        }>;
    }>, {
        nullable: true;
    }>;
    top_logprobs: S.optionalWith<S.filter<S.filter<typeof S.Int>>, {
        nullable: true;
    }>;
    response_format: S.optionalWith<S.Union<[typeof ResponseFormatText, typeof ResponseFormatJsonSchema, typeof ResponseFormatJsonObject]>, {
        nullable: true;
    }>;
    service_tier: S.optionalWith<typeof CreateChatCompletionRequestServiceTier, {
        nullable: true;
        default: () => "auto";
    }>;
    audio: S.optionalWith<S.Struct<{
        voice: typeof VoiceIdsShared;
        format: typeof CreateChatCompletionRequestAudioFormat;
    }>, {
        nullable: true;
    }>;
    store: S.optionalWith<typeof S.Boolean, {
        nullable: true;
        default: () => false;
    }>;
    stream: S.optionalWith<typeof S.Boolean, {
        nullable: true;
        default: () => false;
    }>;
    stop: S.optionalWith<S.NullOr<typeof StopConfiguration>, {
        default: () => null;
    }>;
    logit_bias: S.optionalWith<S.NullOr<S.Record$<typeof S.String, typeof S.Unknown>>, {
        default: () => null;
    }>;
    logprobs: S.optionalWith<typeof S.Boolean, {
        nullable: true;
        default: () => false;
    }>;
    max_tokens: S.optionalWith<typeof S.Int, {
        nullable: true;
    }>;
    n: S.optionalWith<S.filter<S.filter<typeof S.Int>>, {
        nullable: true;
        default: () => 1;
    }>;
    prediction: S.optionalWith<typeof PredictionContent, {
        nullable: true;
    }>;
    seed: S.optionalWith<typeof S.Int, {
        nullable: true;
    }>;
    stream_options: S.optionalWith<S.NullOr<typeof ChatCompletionStreamOptions>, {
        default: () => null;
    }>;
    tools: S.optionalWith<S.Array$<typeof ChatCompletionTool>, {
        nullable: true;
    }>;
    tool_choice: S.optionalWith<typeof ChatCompletionToolChoiceOption, {
        nullable: true;
    }>;
    parallel_tool_calls: S.optionalWith<typeof ParallelToolCalls, {
        nullable: true;
        default: () => true;
    }>;
    function_call: S.optionalWith<S.Union<[typeof CreateChatCompletionRequestFunctionCallEnum, typeof ChatCompletionFunctionCallOption]>, {
        nullable: true;
    }>;
    functions: S.optionalWith<S.filter<S.filter<S.Array$<typeof ChatCompletionFunctions>>>, {
        nullable: true;
    }>;
    metadata: S.optionalWith<typeof Metadata, {
        nullable: true;
    }>;
    temperature: S.optionalWith<S.filter<S.filter<typeof S.Number>>, {
        nullable: true;
        default: () => 1;
    }>;
    top_p: S.optionalWith<S.filter<S.filter<typeof S.Number>>, {
        nullable: true;
        default: () => 1;
    }>;
    user: S.optionalWith<typeof S.String, {
        nullable: true;
    }>;
}>, never, {
    readonly model: string;
} & {
    readonly messages: readonly [{
        readonly content: string | readonly [{
            readonly type: "text";
            readonly text: string;
        }, ...{
            readonly type: "text";
            readonly text: string;
        }[]];
        readonly role: "developer";
        readonly name?: string | undefined;
    } | {
        readonly content: string | readonly [{
            readonly type: "text";
            readonly text: string;
        }, ...{
            readonly type: "text";
            readonly text: string;
        }[]];
        readonly role: "system";
        readonly name?: string | undefined;
    } | {
        readonly content: string | readonly [{
            readonly type: "text";
            readonly text: string;
        } | {
            readonly type: "image_url";
            readonly image_url: {
                readonly url: string;
                readonly detail: "auto" | "low" | "high";
            };
        } | {
            readonly type: "input_audio";
            readonly input_audio: {
                readonly data: string;
                readonly format: "mp3" | "wav";
            };
        } | {
            readonly type: "file";
            readonly file: {
                readonly filename?: string | undefined;
                readonly file_data?: string | undefined;
                readonly file_id?: string | undefined;
            };
        }, ...({
            readonly type: "text";
            readonly text: string;
        } | {
            readonly type: "image_url";
            readonly image_url: {
                readonly url: string;
                readonly detail: "auto" | "low" | "high";
            };
        } | {
            readonly type: "input_audio";
            readonly input_audio: {
                readonly data: string;
                readonly format: "mp3" | "wav";
            };
        } | {
            readonly type: "file";
            readonly file: {
                readonly filename?: string | undefined;
                readonly file_data?: string | undefined;
                readonly file_id?: string | undefined;
            };
        })[]];
        readonly role: "user";
        readonly name?: string | undefined;
    } | {
        readonly content?: string | readonly [{
            readonly type: "text";
            readonly text: string;
        } | {
            readonly type: "refusal";
            readonly refusal: string;
        }, ...({
            readonly type: "text";
            readonly text: string;
        } | {
            readonly type: "refusal";
            readonly refusal: string;
        })[]] | undefined;
        readonly role: "assistant";
        readonly name?: string | undefined;
        readonly refusal?: string | undefined;
        readonly tool_calls?: readonly {
            readonly function: {
                readonly name: string;
                readonly arguments: string;
            };
            readonly type: "function";
            readonly id: string;
        }[] | undefined;
        readonly function_call?: {
            readonly name: string;
            readonly arguments: string;
        } | undefined;
        readonly audio?: {
            readonly id: string;
        } | undefined;
    } | {
        readonly content: string | readonly [{
            readonly type: "text";
            readonly text: string;
        }, ...{
            readonly type: "text";
            readonly text: string;
        }[]];
        readonly role: "tool";
        readonly tool_call_id: string;
    } | {
        readonly content: string | null;
        readonly role: "function";
        readonly name: string;
    }, ...({
        readonly content: string | readonly [{
            readonly type: "text";
            readonly text: string;
        }, ...{
            readonly type: "text";
            readonly text: string;
        }[]];
        readonly role: "developer";
        readonly name?: string | undefined;
    } | {
        readonly content: string | readonly [{
            readonly type: "text";
            readonly text: string;
        }, ...{
            readonly type: "text";
            readonly text: string;
        }[]];
        readonly role: "system";
        readonly name?: string | undefined;
    } | {
        readonly content: string | readonly [{
            readonly type: "text";
            readonly text: string;
        } | {
            readonly type: "image_url";
            readonly image_url: {
                readonly url: string;
                readonly detail: "auto" | "low" | "high";
            };
        } | {
            readonly type: "input_audio";
            readonly input_audio: {
                readonly data: string;
                readonly format: "mp3" | "wav";
            };
        } | {
            readonly type: "file";
            readonly file: {
                readonly filename?: string | undefined;
                readonly file_data?: string | undefined;
                readonly file_id?: string | undefined;
            };
        }, ...({
            readonly type: "text";
            readonly text: string;
        } | {
            readonly type: "image_url";
            readonly image_url: {
                readonly url: string;
                readonly detail: "auto" | "low" | "high";
            };
        } | {
            readonly type: "input_audio";
            readonly input_audio: {
                readonly data: string;
                readonly format: "mp3" | "wav";
            };
        } | {
            readonly type: "file";
            readonly file: {
                readonly filename?: string | undefined;
                readonly file_data?: string | undefined;
                readonly file_id?: string | undefined;
            };
        })[]];
        readonly role: "user";
        readonly name?: string | undefined;
    } | {
        readonly content?: string | readonly [{
            readonly type: "text";
            readonly text: string;
        } | {
            readonly type: "refusal";
            readonly refusal: string;
        }, ...({
            readonly type: "text";
            readonly text: string;
        } | {
            readonly type: "refusal";
            readonly refusal: string;
        })[]] | undefined;
        readonly role: "assistant";
        readonly name?: string | undefined;
        readonly refusal?: string | undefined;
        readonly tool_calls?: readonly {
            readonly function: {
                readonly name: string;
                readonly arguments: string;
            };
            readonly type: "function";
            readonly id: string;
        }[] | undefined;
        readonly function_call?: {
            readonly name: string;
            readonly arguments: string;
        } | undefined;
        readonly audio?: {
            readonly id: string;
        } | undefined;
    } | {
        readonly content: string | readonly [{
            readonly type: "text";
            readonly text: string;
        }, ...{
            readonly type: "text";
            readonly text: string;
        }[]];
        readonly role: "tool";
        readonly tool_call_id: string;
    } | {
        readonly content: string | null;
        readonly role: "function";
        readonly name: string;
    })[]];
} & {
    readonly tools?: readonly {
        readonly function: {
            readonly description?: string | undefined;
            readonly name: string;
            readonly parameters?: {
                readonly [x: string]: unknown;
            } | undefined;
            readonly strict: boolean;
        };
        readonly type: "function";
    }[] | undefined;
} & {
    readonly metadata?: {
        readonly [x: string]: unknown;
    } | undefined;
} & {
    readonly temperature?: number;
} & {
    readonly top_p?: number;
} & {
    readonly response_format?: {
        readonly type: "text";
    } | {
        readonly type: "json_object";
    } | {
        readonly type: "json_schema";
        readonly json_schema: {
            readonly description?: string | undefined;
            readonly name: string;
            readonly strict: boolean;
            readonly schema?: {
                readonly [x: string]: unknown;
            } | undefined;
        };
    } | undefined;
} & {
    readonly reasoning_effort?: "low" | "medium" | "high";
} & {
    readonly logprobs?: boolean;
} & {
    readonly function_call?: "none" | "auto" | {
        readonly name: string;
    } | undefined;
} & {
    readonly audio?: {
        readonly voice: string;
        readonly format: "mp3" | "opus" | "flac" | "wav" | "pcm16";
    } | undefined;
} & {
    readonly top_logprobs?: number | undefined;
} & {
    readonly stop?: string | readonly string[] | null;
} & {
    readonly service_tier?: "default" | "auto";
} & {
    readonly user?: string | undefined;
} & {
    readonly modalities?: readonly ("text" | "audio")[] | undefined;
} & {
    readonly max_completion_tokens?: number | undefined;
} & {
    readonly frequency_penalty?: number;
} & {
    readonly presence_penalty?: number;
} & {
    readonly web_search_options?: {
        readonly user_location?: {
            readonly type: "approximate";
            readonly approximate: {
                readonly country?: string | undefined;
                readonly region?: string | undefined;
                readonly city?: string | undefined;
                readonly timezone?: string | undefined;
            };
        } | undefined;
        readonly search_context_size: "low" | "medium" | "high";
    } | undefined;
} & {
    readonly store?: boolean;
} & {
    readonly stream?: boolean;
} & {
    readonly logit_bias?: {
        readonly [x: string]: unknown;
    } | null;
} & {
    readonly max_tokens?: number | undefined;
} & {
    readonly n?: number;
} & {
    readonly prediction?: {
        readonly content: string | readonly [{
            readonly type: "text";
            readonly text: string;
        }, ...{
            readonly type: "text";
            readonly text: string;
        }[]];
        readonly type: "content";
    } | undefined;
} & {
    readonly seed?: number | undefined;
} & {
    readonly stream_options?: {
        readonly include_usage?: boolean | undefined;
    } | null;
} & {
    readonly tool_choice?: "none" | "auto" | "required" | {
        readonly function: {
            readonly name: string;
        };
        readonly type: "function";
    } | undefined;
} & {
    readonly parallel_tool_calls?: boolean;
} & {
    readonly functions?: readonly {
        readonly description?: string | undefined;
        readonly name: string;
        readonly parameters?: {
            readonly [x: string]: unknown;
        } | undefined;
    }[] | undefined;
}, {}, {}>;
export declare class CreateChatCompletionRequest extends CreateChatCompletionRequest_base {
}
declare const UpdateChatCompletionRequest_base: S.Class<UpdateChatCompletionRequest, {
    metadata: S.NullOr<typeof Metadata>;
}, S.Struct.Encoded<{
    metadata: S.NullOr<typeof Metadata>;
}>, never, {
    readonly metadata: {
        readonly [x: string]: unknown;
    } | null;
}, {}, {}>;
export declare class UpdateChatCompletionRequest extends UpdateChatCompletionRequest_base {
}
declare const ChatCompletionDeletedObject_base: S.Literal<["chat.completion.deleted"]>;
export declare class ChatCompletionDeletedObject extends ChatCompletionDeletedObject_base {
}
declare const ChatCompletionDeleted_base: S.Class<ChatCompletionDeleted, {
    object: typeof ChatCompletionDeletedObject;
    id: typeof S.String;
    deleted: typeof S.Boolean;
}, S.Struct.Encoded<{
    object: typeof ChatCompletionDeletedObject;
    id: typeof S.String;
    deleted: typeof S.Boolean;
}>, never, {
    readonly object: "chat.completion.deleted";
} & {
    readonly id: string;
} & {
    readonly deleted: boolean;
}, {}, {}>;
export declare class ChatCompletionDeleted extends ChatCompletionDeleted_base {
}
declare const GetChatCompletionMessagesParamsOrder_base: S.Literal<["asc", "desc"]>;
export declare class GetChatCompletionMessagesParamsOrder extends GetChatCompletionMessagesParamsOrder_base {
}
declare const GetChatCompletionMessagesParams_base: S.Struct<{
    after: S.optionalWith<typeof S.String, {
        nullable: true;
    }>;
    limit: S.optionalWith<typeof S.Int, {
        nullable: true;
        default: () => 20;
    }>;
    order: S.optionalWith<typeof GetChatCompletionMessagesParamsOrder, {
        nullable: true;
        default: () => "asc";
    }>;
}>;
export declare class GetChatCompletionMessagesParams extends GetChatCompletionMessagesParams_base {
}
declare const ChatCompletionMessageListObject_base: S.Literal<["list"]>;
export declare class ChatCompletionMessageListObject extends ChatCompletionMessageListObject_base {
}
declare const ChatCompletionMessageList_base: S.Class<ChatCompletionMessageList, {
    object: S.PropertySignature<":", "list", never, ":", "list", true, never>;
    data: S.Array$<S.Struct<{
        id: typeof S.String;
        content: S.NullOr<typeof S.String>;
        refusal: S.NullOr<typeof S.String>;
        tool_calls: S.optionalWith<typeof ChatCompletionMessageToolCalls, {
            nullable: true;
        }>;
        annotations: S.optionalWith<S.Array$<S.Struct<{
            type: S.Literal<["url_citation"]>;
            url_citation: S.Struct<{
                end_index: typeof S.Int;
                start_index: typeof S.Int;
                url: typeof S.String;
                title: typeof S.String;
            }>;
        }>>, {
            nullable: true;
        }>;
        role: S.Literal<["assistant"]>;
        function_call: S.optionalWith<S.Struct<{
            arguments: typeof S.String;
            name: typeof S.String;
        }>, {
            nullable: true;
        }>;
        audio: S.optionalWith<S.Struct<{
            id: typeof S.String;
            expires_at: typeof S.Int;
            data: typeof S.String;
            transcript: typeof S.String;
        }>, {
            nullable: true;
        }>;
    }>>;
    first_id: typeof S.String;
    last_id: typeof S.String;
    has_more: typeof S.Boolean;
}, S.Struct.Encoded<{
    object: S.PropertySignature<":", "list", never, ":", "list", true, never>;
    data: S.Array$<S.Struct<{
        id: typeof S.String;
        content: S.NullOr<typeof S.String>;
        refusal: S.NullOr<typeof S.String>;
        tool_calls: S.optionalWith<typeof ChatCompletionMessageToolCalls, {
            nullable: true;
        }>;
        annotations: S.optionalWith<S.Array$<S.Struct<{
            type: S.Literal<["url_citation"]>;
            url_citation: S.Struct<{
                end_index: typeof S.Int;
                start_index: typeof S.Int;
                url: typeof S.String;
                title: typeof S.String;
            }>;
        }>>, {
            nullable: true;
        }>;
        role: S.Literal<["assistant"]>;
        function_call: S.optionalWith<S.Struct<{
            arguments: typeof S.String;
            name: typeof S.String;
        }>, {
            nullable: true;
        }>;
        audio: S.optionalWith<S.Struct<{
            id: typeof S.String;
            expires_at: typeof S.Int;
            data: typeof S.String;
            transcript: typeof S.String;
        }>, {
            nullable: true;
        }>;
    }>>;
    first_id: typeof S.String;
    last_id: typeof S.String;
    has_more: typeof S.Boolean;
}>, never, {
    readonly object?: "list";
} & {
    readonly first_id: string;
} & {
    readonly last_id: string;
} & {
    readonly has_more: boolean;
} & {
    readonly data: readonly {
        readonly annotations?: readonly {
            readonly type: "url_citation";
            readonly url_citation: {
                readonly end_index: number;
                readonly start_index: number;
                readonly url: string;
                readonly title: string;
            };
        }[] | undefined;
        readonly content: string | null;
        readonly role: "assistant";
        readonly id: string;
        readonly refusal: string | null;
        readonly tool_calls?: readonly {
            readonly function: {
                readonly name: string;
                readonly arguments: string;
            };
            readonly type: "function";
            readonly id: string;
        }[] | undefined;
        readonly function_call?: {
            readonly name: string;
            readonly arguments: string;
        } | undefined;
        readonly audio?: {
            readonly id: string;
            readonly data: string;
            readonly expires_at: number;
            readonly transcript: string;
        } | undefined;
    }[];
}, {}, {}>;
export declare class ChatCompletionMessageList extends ChatCompletionMessageList_base {
}
declare const CreateCompletionRequestModelEnum_base: S.Literal<["gpt-3.5-turbo-instruct", "davinci-002", "babbage-002"]>;
export declare class CreateCompletionRequestModelEnum extends CreateCompletionRequestModelEnum_base {
}
declare const CreateCompletionRequest_base: S.Class<CreateCompletionRequest, {
    model: S.Union<[typeof S.String, typeof CreateCompletionRequestModelEnum]>;
    prompt: S.PropertySignature<":", string | readonly string[] | readonly [number, ...number[]] | readonly [readonly [number, ...number[]], ...(readonly [number, ...number[]])[]] | null, never, ":", string | readonly string[] | readonly [number, ...number[]] | readonly [readonly [number, ...number[]], ...(readonly [number, ...number[]])[]] | null, true, never>;
    best_of: S.optionalWith<S.filter<S.filter<typeof S.Int>>, {
        nullable: true;
        default: () => 1;
    }>;
    echo: S.optionalWith<typeof S.Boolean, {
        nullable: true;
        default: () => false;
    }>;
    frequency_penalty: S.optionalWith<S.filter<S.filter<typeof S.Number>>, {
        nullable: true;
        default: () => 0;
    }>;
    logit_bias: S.optionalWith<S.NullOr<S.Record$<typeof S.String, typeof S.Unknown>>, {
        default: () => null;
    }>;
    logprobs: S.optionalWith<S.NullOr<S.filter<S.filter<typeof S.Int>>>, {
        default: () => null;
    }>;
    max_tokens: S.optionalWith<S.filter<typeof S.Int>, {
        nullable: true;
        default: () => 16;
    }>;
    n: S.optionalWith<S.filter<S.filter<typeof S.Int>>, {
        nullable: true;
        default: () => 1;
    }>;
    presence_penalty: S.optionalWith<S.filter<S.filter<typeof S.Number>>, {
        nullable: true;
        default: () => 0;
    }>;
    seed: S.optionalWith<typeof S.Int, {
        nullable: true;
    }>;
    stop: S.optionalWith<S.NullOr<typeof StopConfiguration>, {
        default: () => null;
    }>;
    stream: S.optionalWith<typeof S.Boolean, {
        nullable: true;
        default: () => false;
    }>;
    stream_options: S.optionalWith<S.NullOr<typeof ChatCompletionStreamOptions>, {
        default: () => null;
    }>;
    suffix: S.optionalWith<S.NullOr<typeof S.String>, {
        default: () => null;
    }>;
    temperature: S.optionalWith<S.filter<S.filter<typeof S.Number>>, {
        nullable: true;
        default: () => 1;
    }>;
    top_p: S.optionalWith<S.filter<S.filter<typeof S.Number>>, {
        nullable: true;
        default: () => 1;
    }>;
    user: S.optionalWith<typeof S.String, {
        nullable: true;
    }>;
}, S.Struct.Encoded<{
    model: S.Union<[typeof S.String, typeof CreateCompletionRequestModelEnum]>;
    prompt: S.PropertySignature<":", string | readonly string[] | readonly [number, ...number[]] | readonly [readonly [number, ...number[]], ...(readonly [number, ...number[]])[]] | null, never, ":", string | readonly string[] | readonly [number, ...number[]] | readonly [readonly [number, ...number[]], ...(readonly [number, ...number[]])[]] | null, true, never>;
    best_of: S.optionalWith<S.filter<S.filter<typeof S.Int>>, {
        nullable: true;
        default: () => 1;
    }>;
    echo: S.optionalWith<typeof S.Boolean, {
        nullable: true;
        default: () => false;
    }>;
    frequency_penalty: S.optionalWith<S.filter<S.filter<typeof S.Number>>, {
        nullable: true;
        default: () => 0;
    }>;
    logit_bias: S.optionalWith<S.NullOr<S.Record$<typeof S.String, typeof S.Unknown>>, {
        default: () => null;
    }>;
    logprobs: S.optionalWith<S.NullOr<S.filter<S.filter<typeof S.Int>>>, {
        default: () => null;
    }>;
    max_tokens: S.optionalWith<S.filter<typeof S.Int>, {
        nullable: true;
        default: () => 16;
    }>;
    n: S.optionalWith<S.filter<S.filter<typeof S.Int>>, {
        nullable: true;
        default: () => 1;
    }>;
    presence_penalty: S.optionalWith<S.filter<S.filter<typeof S.Number>>, {
        nullable: true;
        default: () => 0;
    }>;
    seed: S.optionalWith<typeof S.Int, {
        nullable: true;
    }>;
    stop: S.optionalWith<S.NullOr<typeof StopConfiguration>, {
        default: () => null;
    }>;
    stream: S.optionalWith<typeof S.Boolean, {
        nullable: true;
        default: () => false;
    }>;
    stream_options: S.optionalWith<S.NullOr<typeof ChatCompletionStreamOptions>, {
        default: () => null;
    }>;
    suffix: S.optionalWith<S.NullOr<typeof S.String>, {
        default: () => null;
    }>;
    temperature: S.optionalWith<S.filter<S.filter<typeof S.Number>>, {
        nullable: true;
        default: () => 1;
    }>;
    top_p: S.optionalWith<S.filter<S.filter<typeof S.Number>>, {
        nullable: true;
        default: () => 1;
    }>;
    user: S.optionalWith<typeof S.String, {
        nullable: true;
    }>;
}>, never, {
    readonly model: string;
} & {
    readonly temperature?: number;
} & {
    readonly top_p?: number;
} & {
    readonly echo?: boolean;
} & {
    readonly logprobs?: number | null;
} & {
    readonly stop?: string | readonly string[] | null;
} & {
    readonly user?: string | undefined;
} & {
    readonly frequency_penalty?: number;
} & {
    readonly presence_penalty?: number;
} & {
    readonly stream?: boolean;
} & {
    readonly logit_bias?: {
        readonly [x: string]: unknown;
    } | null;
} & {
    readonly max_tokens?: number;
} & {
    readonly n?: number;
} & {
    readonly seed?: number | undefined;
} & {
    readonly stream_options?: {
        readonly include_usage?: boolean | undefined;
    } | null;
} & {
    readonly prompt?: string | readonly string[] | readonly [number, ...number[]] | readonly [readonly [number, ...number[]], ...(readonly [number, ...number[]])[]] | null;
} & {
    readonly best_of?: number;
} & {
    readonly suffix?: string | null;
}, {}, {}>;
export declare class CreateCompletionRequest extends CreateCompletionRequest_base {
}
declare const CreateCompletionResponseObject_base: S.Literal<["text_completion"]>;
export declare class CreateCompletionResponseObject extends CreateCompletionResponseObject_base {
}
declare const CreateCompletionResponse_base: S.Class<CreateCompletionResponse, {
    id: typeof S.String;
    choices: S.Array$<S.Struct<{
        finish_reason: S.Literal<["stop", "length", "content_filter"]>;
        index: typeof S.Int;
        logprobs: S.NullOr<S.Struct<{
            text_offset: S.optionalWith<S.Array$<typeof S.Int>, {
                nullable: true;
            }>;
            token_logprobs: S.optionalWith<S.Array$<typeof S.Number>, {
                nullable: true;
            }>;
            tokens: S.optionalWith<S.Array$<typeof S.String>, {
                nullable: true;
            }>;
            top_logprobs: S.optionalWith<S.Array$<S.Record$<typeof S.String, typeof S.Unknown>>, {
                nullable: true;
            }>;
        }>>;
        text: typeof S.String;
    }>>;
    created: typeof S.Int;
    model: typeof S.String;
    system_fingerprint: S.optionalWith<typeof S.String, {
        nullable: true;
    }>;
    object: typeof CreateCompletionResponseObject;
    usage: S.optionalWith<typeof CompletionUsage, {
        nullable: true;
    }>;
}, S.Struct.Encoded<{
    id: typeof S.String;
    choices: S.Array$<S.Struct<{
        finish_reason: S.Literal<["stop", "length", "content_filter"]>;
        index: typeof S.Int;
        logprobs: S.NullOr<S.Struct<{
            text_offset: S.optionalWith<S.Array$<typeof S.Int>, {
                nullable: true;
            }>;
            token_logprobs: S.optionalWith<S.Array$<typeof S.Number>, {
                nullable: true;
            }>;
            tokens: S.optionalWith<S.Array$<typeof S.String>, {
                nullable: true;
            }>;
            top_logprobs: S.optionalWith<S.Array$<S.Record$<typeof S.String, typeof S.Unknown>>, {
                nullable: true;
            }>;
        }>>;
        text: typeof S.String;
    }>>;
    created: typeof S.Int;
    model: typeof S.String;
    system_fingerprint: S.optionalWith<typeof S.String, {
        nullable: true;
    }>;
    object: typeof CreateCompletionResponseObject;
    usage: S.optionalWith<typeof CompletionUsage, {
        nullable: true;
    }>;
}>, never, {
    readonly object: "text_completion";
} & {
    readonly model: string;
} & {
    readonly id: string;
} & {
    readonly choices: readonly {
        readonly text: string;
        readonly logprobs: {
            readonly tokens?: readonly string[] | undefined;
            readonly top_logprobs?: readonly {
                readonly [x: string]: unknown;
            }[] | undefined;
            readonly text_offset?: readonly number[] | undefined;
            readonly token_logprobs?: readonly number[] | undefined;
        } | null;
        readonly finish_reason: "length" | "stop" | "content_filter";
        readonly index: number;
    }[];
} & {
    readonly created: number;
} & {
    readonly system_fingerprint?: string | undefined;
} & {
    readonly usage?: {
        readonly completion_tokens: number;
        readonly prompt_tokens: number;
        readonly total_tokens: number;
        readonly completion_tokens_details?: {
            readonly accepted_prediction_tokens: number;
            readonly audio_tokens: number;
            readonly reasoning_tokens: number;
            readonly rejected_prediction_tokens: number;
        } | undefined;
        readonly prompt_tokens_details?: {
            readonly audio_tokens: number;
            readonly cached_tokens: number;
        } | undefined;
    } | undefined;
}, {}, {}>;
export declare class CreateCompletionResponse extends CreateCompletionResponse_base {
}
declare const CreateEmbeddingRequestModelEnum_base: S.Literal<["text-embedding-ada-002", "text-embedding-3-small", "text-embedding-3-large"]>;
export declare class CreateEmbeddingRequestModelEnum extends CreateEmbeddingRequestModelEnum_base {
}
declare const CreateEmbeddingRequestEncodingFormat_base: S.Literal<["float", "base64"]>;
export declare class CreateEmbeddingRequestEncodingFormat extends CreateEmbeddingRequestEncodingFormat_base {
}
declare const CreateEmbeddingRequest_base: S.Class<CreateEmbeddingRequest, {
    input: S.Union<[typeof S.String, S.filter<S.filter<S.Array$<typeof S.String>>>, S.filter<S.filter<S.Array$<typeof S.Int>>>, S.filter<S.filter<S.Array$<S.NonEmptyArray<typeof S.Int>>>>]>;
    model: S.Union<[typeof S.String, typeof CreateEmbeddingRequestModelEnum]>;
    encoding_format: S.optionalWith<typeof CreateEmbeddingRequestEncodingFormat, {
        nullable: true;
        default: () => "float";
    }>;
    dimensions: S.optionalWith<S.filter<typeof S.Int>, {
        nullable: true;
    }>;
    user: S.optionalWith<typeof S.String, {
        nullable: true;
    }>;
}, S.Struct.Encoded<{
    input: S.Union<[typeof S.String, S.filter<S.filter<S.Array$<typeof S.String>>>, S.filter<S.filter<S.Array$<typeof S.Int>>>, S.filter<S.filter<S.Array$<S.NonEmptyArray<typeof S.Int>>>>]>;
    model: S.Union<[typeof S.String, typeof CreateEmbeddingRequestModelEnum]>;
    encoding_format: S.optionalWith<typeof CreateEmbeddingRequestEncodingFormat, {
        nullable: true;
        default: () => "float";
    }>;
    dimensions: S.optionalWith<S.filter<typeof S.Int>, {
        nullable: true;
    }>;
    user: S.optionalWith<typeof S.String, {
        nullable: true;
    }>;
}>, never, {
    readonly model: string;
} & {
    readonly dimensions?: number | undefined;
} & {
    readonly input: string | readonly number[] | readonly string[] | readonly (readonly [number, ...number[]])[];
} & {
    readonly user?: string | undefined;
} & {
    readonly encoding_format?: "float" | "base64";
}, {}, {}>;
export declare class CreateEmbeddingRequest extends CreateEmbeddingRequest_base {
}
declare const EmbeddingObject_base: S.Literal<["embedding"]>;
export declare class EmbeddingObject extends EmbeddingObject_base {
}
declare const Embedding_base: S.Struct<{
    index: typeof S.Int;
    embedding: S.Array$<typeof S.Number>;
    object: typeof EmbeddingObject;
}>;
export declare class Embedding extends Embedding_base {
}
declare const CreateEmbeddingResponseObject_base: S.Literal<["list"]>;
export declare class CreateEmbeddingResponseObject extends CreateEmbeddingResponseObject_base {
}
declare const CreateEmbeddingResponse_base: S.Class<CreateEmbeddingResponse, {
    data: S.Array$<typeof Embedding>;
    model: typeof S.String;
    object: typeof CreateEmbeddingResponseObject;
    usage: S.Struct<{
        prompt_tokens: typeof S.Int;
        total_tokens: typeof S.Int;
    }>;
}, S.Struct.Encoded<{
    data: S.Array$<typeof Embedding>;
    model: typeof S.String;
    object: typeof CreateEmbeddingResponseObject;
    usage: S.Struct<{
        prompt_tokens: typeof S.Int;
        total_tokens: typeof S.Int;
    }>;
}>, never, {
    readonly object: "list";
} & {
    readonly model: string;
} & {
    readonly data: readonly {
        readonly object: "embedding";
        readonly embedding: readonly number[];
        readonly index: number;
    }[];
} & {
    readonly usage: {
        readonly prompt_tokens: number;
        readonly total_tokens: number;
    };
}, {}, {}>;
export declare class CreateEmbeddingResponse extends CreateEmbeddingResponse_base {
}
declare const ListFilesParamsOrder_base: S.Literal<["asc", "desc"]>;
export declare class ListFilesParamsOrder extends ListFilesParamsOrder_base {
}
declare const ListFilesParams_base: S.Struct<{
    purpose: S.optionalWith<typeof S.String, {
        nullable: true;
    }>;
    limit: S.optionalWith<typeof S.Int, {
        nullable: true;
        default: () => 10000;
    }>;
    order: S.optionalWith<typeof ListFilesParamsOrder, {
        nullable: true;
        default: () => "desc";
    }>;
    after: S.optionalWith<typeof S.String, {
        nullable: true;
    }>;
}>;
export declare class ListFilesParams extends ListFilesParams_base {
}
declare const OpenAIFileObject_base: S.Literal<["file"]>;
export declare class OpenAIFileObject extends OpenAIFileObject_base {
}
declare const OpenAIFilePurpose_base: S.Literal<["assistants", "assistants_output", "batch", "batch_output", "fine-tune", "fine-tune-results", "vision"]>;
export declare class OpenAIFilePurpose extends OpenAIFilePurpose_base {
}
declare const OpenAIFileStatus_base: S.Literal<["uploaded", "processed", "error"]>;
export declare class OpenAIFileStatus extends OpenAIFileStatus_base {
}
declare const OpenAIFile_base: S.Struct<{
    id: typeof S.String;
    bytes: typeof S.Int;
    created_at: typeof S.Int;
    expires_at: S.optionalWith<typeof S.Int, {
        nullable: true;
    }>;
    filename: typeof S.String;
    object: typeof OpenAIFileObject;
    purpose: typeof OpenAIFilePurpose;
    status: typeof OpenAIFileStatus;
    status_details: S.optionalWith<typeof S.String, {
        nullable: true;
    }>;
}>;
export declare class OpenAIFile extends OpenAIFile_base {
}
declare const ListFilesResponse_base: S.Class<ListFilesResponse, {
    object: typeof S.String;
    data: S.Array$<typeof OpenAIFile>;
    first_id: typeof S.String;
    last_id: typeof S.String;
    has_more: typeof S.Boolean;
}, S.Struct.Encoded<{
    object: typeof S.String;
    data: S.Array$<typeof OpenAIFile>;
    first_id: typeof S.String;
    last_id: typeof S.String;
    has_more: typeof S.Boolean;
}>, never, {
    readonly object: string;
} & {
    readonly first_id: string;
} & {
    readonly last_id: string;
} & {
    readonly has_more: boolean;
} & {
    readonly data: readonly {
        readonly object: "file";
        readonly id: string;
        readonly created_at: number;
        readonly bytes: number;
        readonly status: "uploaded" | "processed" | "error";
        readonly expires_at?: number | undefined;
        readonly filename: string;
        readonly purpose: "batch" | "assistants" | "assistants_output" | "batch_output" | "fine-tune" | "fine-tune-results" | "vision";
        readonly status_details?: string | undefined;
    }[];
}, {}, {}>;
export declare class ListFilesResponse extends ListFilesResponse_base {
}
declare const DeleteFileResponseObject_base: S.Literal<["file"]>;
export declare class DeleteFileResponseObject extends DeleteFileResponseObject_base {
}
declare const DeleteFileResponse_base: S.Class<DeleteFileResponse, {
    id: typeof S.String;
    object: typeof DeleteFileResponseObject;
    deleted: typeof S.Boolean;
}, S.Struct.Encoded<{
    id: typeof S.String;
    object: typeof DeleteFileResponseObject;
    deleted: typeof S.Boolean;
}>, never, {
    readonly object: "file";
} & {
    readonly id: string;
} & {
    readonly deleted: boolean;
}, {}, {}>;
export declare class DeleteFileResponse extends DeleteFileResponse_base {
}
export declare class DownloadFile200 extends S.String {
}
declare const ListFineTuningCheckpointPermissionsParamsOrder_base: S.Literal<["ascending", "descending"]>;
export declare class ListFineTuningCheckpointPermissionsParamsOrder extends ListFineTuningCheckpointPermissionsParamsOrder_base {
}
declare const ListFineTuningCheckpointPermissionsParams_base: S.Struct<{
    project_id: S.optionalWith<typeof S.String, {
        nullable: true;
    }>;
    after: S.optionalWith<typeof S.String, {
        nullable: true;
    }>;
    limit: S.optionalWith<typeof S.Int, {
        nullable: true;
        default: () => 10;
    }>;
    order: S.optionalWith<typeof ListFineTuningCheckpointPermissionsParamsOrder, {
        nullable: true;
        default: () => "descending";
    }>;
}>;
export declare class ListFineTuningCheckpointPermissionsParams extends ListFineTuningCheckpointPermissionsParams_base {
}
declare const FineTuningCheckpointPermissionObject_base: S.Literal<["checkpoint.permission"]>;
export declare class FineTuningCheckpointPermissionObject extends FineTuningCheckpointPermissionObject_base {
}
declare const FineTuningCheckpointPermission_base: S.Struct<{
    id: typeof S.String;
    created_at: typeof S.Int;
    project_id: typeof S.String;
    object: typeof FineTuningCheckpointPermissionObject;
}>;
export declare class FineTuningCheckpointPermission extends FineTuningCheckpointPermission_base {
}
declare const ListFineTuningCheckpointPermissionResponseObject_base: S.Literal<["list"]>;
export declare class ListFineTuningCheckpointPermissionResponseObject extends ListFineTuningCheckpointPermissionResponseObject_base {
}
declare const ListFineTuningCheckpointPermissionResponse_base: S.Class<ListFineTuningCheckpointPermissionResponse, {
    data: S.Array$<typeof FineTuningCheckpointPermission>;
    object: typeof ListFineTuningCheckpointPermissionResponseObject;
    first_id: S.optionalWith<typeof S.String, {
        nullable: true;
    }>;
    last_id: S.optionalWith<typeof S.String, {
        nullable: true;
    }>;
    has_more: typeof S.Boolean;
}, S.Struct.Encoded<{
    data: S.Array$<typeof FineTuningCheckpointPermission>;
    object: typeof ListFineTuningCheckpointPermissionResponseObject;
    first_id: S.optionalWith<typeof S.String, {
        nullable: true;
    }>;
    last_id: S.optionalWith<typeof S.String, {
        nullable: true;
    }>;
    has_more: typeof S.Boolean;
}>, never, {
    readonly object: "list";
} & {
    readonly first_id?: string | undefined;
} & {
    readonly last_id?: string | undefined;
} & {
    readonly has_more: boolean;
} & {
    readonly data: readonly {
        readonly object: "checkpoint.permission";
        readonly id: string;
        readonly created_at: number;
        readonly project_id: string;
    }[];
}, {}, {}>;
export declare class ListFineTuningCheckpointPermissionResponse extends ListFineTuningCheckpointPermissionResponse_base {
}
declare const CreateFineTuningCheckpointPermissionRequest_base: S.Class<CreateFineTuningCheckpointPermissionRequest, {
    project_ids: S.Array$<typeof S.String>;
}, S.Struct.Encoded<{
    project_ids: S.Array$<typeof S.String>;
}>, never, {
    readonly project_ids: readonly string[];
}, {}, {}>;
export declare class CreateFineTuningCheckpointPermissionRequest extends CreateFineTuningCheckpointPermissionRequest_base {
}
declare const DeleteFineTuningCheckpointPermissionResponseObject_base: S.Literal<["checkpoint.permission"]>;
export declare class DeleteFineTuningCheckpointPermissionResponseObject extends DeleteFineTuningCheckpointPermissionResponseObject_base {
}
declare const DeleteFineTuningCheckpointPermissionResponse_base: S.Class<DeleteFineTuningCheckpointPermissionResponse, {
    id: typeof S.String;
    object: typeof DeleteFineTuningCheckpointPermissionResponseObject;
    deleted: typeof S.Boolean;
}, S.Struct.Encoded<{
    id: typeof S.String;
    object: typeof DeleteFineTuningCheckpointPermissionResponseObject;
    deleted: typeof S.Boolean;
}>, never, {
    readonly object: "checkpoint.permission";
} & {
    readonly id: string;
} & {
    readonly deleted: boolean;
}, {}, {}>;
export declare class DeleteFineTuningCheckpointPermissionResponse extends DeleteFineTuningCheckpointPermissionResponse_base {
}
declare const ListPaginatedFineTuningJobsParams_base: S.Struct<{
    after: S.optionalWith<typeof S.String, {
        nullable: true;
    }>;
    limit: S.optionalWith<typeof S.Int, {
        nullable: true;
        default: () => 20;
    }>;
    metadata: S.optionalWith<S.Record$<typeof S.String, typeof S.Unknown>, {
        nullable: true;
    }>;
}>;
export declare class ListPaginatedFineTuningJobsParams extends ListPaginatedFineTuningJobsParams_base {
}
declare const FineTuningJobHyperparametersBatchSizeEnum_base: S.Literal<["auto"]>;
export declare class FineTuningJobHyperparametersBatchSizeEnum extends FineTuningJobHyperparametersBatchSizeEnum_base {
}
declare const FineTuningJobHyperparametersLearningRateMultiplierEnum_base: S.Literal<["auto"]>;
export declare class FineTuningJobHyperparametersLearningRateMultiplierEnum extends FineTuningJobHyperparametersLearningRateMultiplierEnum_base {
}
declare const FineTuningJobHyperparametersNEpochsEnum_base: S.Literal<["auto"]>;
export declare class FineTuningJobHyperparametersNEpochsEnum extends FineTuningJobHyperparametersNEpochsEnum_base {
}
declare const FineTuningJobObject_base: S.Literal<["fine_tuning.job"]>;
export declare class FineTuningJobObject extends FineTuningJobObject_base {
}
declare const FineTuningJobStatus_base: S.Literal<["validating_files", "queued", "running", "succeeded", "failed", "cancelled"]>;
export declare class FineTuningJobStatus extends FineTuningJobStatus_base {
}
declare const FineTuningIntegrationType_base: S.Literal<["wandb"]>;
export declare class FineTuningIntegrationType extends FineTuningIntegrationType_base {
}
declare const FineTuningIntegration_base: S.Struct<{
    type: typeof FineTuningIntegrationType;
    wandb: S.Struct<{
        project: typeof S.String;
        name: S.optionalWith<typeof S.String, {
            nullable: true;
        }>;
        entity: S.optionalWith<typeof S.String, {
            nullable: true;
        }>;
        tags: S.optionalWith<S.Array$<typeof S.String>, {
            nullable: true;
        }>;
    }>;
}>;
export declare class FineTuningIntegration extends FineTuningIntegration_base {
}
declare const FineTuneMethodType_base: S.Literal<["supervised", "dpo"]>;
export declare class FineTuneMethodType extends FineTuneMethodType_base {
}
declare const FineTuneSupervisedMethodHyperparametersBatchSizeEnum_base: S.Literal<["auto"]>;
export declare class FineTuneSupervisedMethodHyperparametersBatchSizeEnum extends FineTuneSupervisedMethodHyperparametersBatchSizeEnum_base {
}
declare const FineTuneSupervisedMethodHyperparametersLearningRateMultiplierEnum_base: S.Literal<["auto"]>;
export declare class FineTuneSupervisedMethodHyperparametersLearningRateMultiplierEnum extends FineTuneSupervisedMethodHyperparametersLearningRateMultiplierEnum_base {
}
declare const FineTuneSupervisedMethodHyperparametersNEpochsEnum_base: S.Literal<["auto"]>;
export declare class FineTuneSupervisedMethodHyperparametersNEpochsEnum extends FineTuneSupervisedMethodHyperparametersNEpochsEnum_base {
}
declare const FineTuneSupervisedMethod_base: S.Struct<{
    hyperparameters: S.optionalWith<S.Struct<{
        batch_size: S.optionalWith<S.Union<[typeof FineTuneSupervisedMethodHyperparametersBatchSizeEnum, S.filter<S.filter<typeof S.Int>>]>, {
            nullable: true;
            default: () => "auto";
        }>;
        learning_rate_multiplier: S.optionalWith<S.Union<[typeof FineTuneSupervisedMethodHyperparametersLearningRateMultiplierEnum, S.filter<typeof S.Number>]>, {
            nullable: true;
            default: () => "auto";
        }>;
        n_epochs: S.optionalWith<S.Union<[typeof FineTuneSupervisedMethodHyperparametersNEpochsEnum, S.filter<S.filter<typeof S.Int>>]>, {
            nullable: true;
            default: () => "auto";
        }>;
    }>, {
        nullable: true;
    }>;
}>;
export declare class FineTuneSupervisedMethod extends FineTuneSupervisedMethod_base {
}
declare const FineTuneDPOMethodHyperparametersBetaEnum_base: S.Literal<["auto"]>;
export declare class FineTuneDPOMethodHyperparametersBetaEnum extends FineTuneDPOMethodHyperparametersBetaEnum_base {
}
declare const FineTuneDPOMethodHyperparametersBatchSizeEnum_base: S.Literal<["auto"]>;
export declare class FineTuneDPOMethodHyperparametersBatchSizeEnum extends FineTuneDPOMethodHyperparametersBatchSizeEnum_base {
}
declare const FineTuneDPOMethodHyperparametersLearningRateMultiplierEnum_base: S.Literal<["auto"]>;
export declare class FineTuneDPOMethodHyperparametersLearningRateMultiplierEnum extends FineTuneDPOMethodHyperparametersLearningRateMultiplierEnum_base {
}
declare const FineTuneDPOMethodHyperparametersNEpochsEnum_base: S.Literal<["auto"]>;
export declare class FineTuneDPOMethodHyperparametersNEpochsEnum extends FineTuneDPOMethodHyperparametersNEpochsEnum_base {
}
declare const FineTuneDPOMethod_base: S.Struct<{
    hyperparameters: S.optionalWith<S.Struct<{
        beta: S.optionalWith<S.Union<[typeof FineTuneDPOMethodHyperparametersBetaEnum, S.filter<S.filter<typeof S.Number>>]>, {
            nullable: true;
            default: () => "auto";
        }>;
        batch_size: S.optionalWith<S.Union<[typeof FineTuneDPOMethodHyperparametersBatchSizeEnum, S.filter<S.filter<typeof S.Int>>]>, {
            nullable: true;
            default: () => "auto";
        }>;
        learning_rate_multiplier: S.optionalWith<S.Union<[typeof FineTuneDPOMethodHyperparametersLearningRateMultiplierEnum, S.filter<typeof S.Number>]>, {
            nullable: true;
            default: () => "auto";
        }>;
        n_epochs: S.optionalWith<S.Union<[typeof FineTuneDPOMethodHyperparametersNEpochsEnum, S.filter<S.filter<typeof S.Int>>]>, {
            nullable: true;
            default: () => "auto";
        }>;
    }>, {
        nullable: true;
    }>;
}>;
export declare class FineTuneDPOMethod extends FineTuneDPOMethod_base {
}
declare const FineTuneMethod_base: S.Struct<{
    type: S.optionalWith<typeof FineTuneMethodType, {
        nullable: true;
    }>;
    supervised: S.optionalWith<typeof FineTuneSupervisedMethod, {
        nullable: true;
    }>;
    dpo: S.optionalWith<typeof FineTuneDPOMethod, {
        nullable: true;
    }>;
}>;
export declare class FineTuneMethod extends FineTuneMethod_base {
}
declare const FineTuningJob_base: S.Struct<{
    id: typeof S.String;
    created_at: typeof S.Int;
    error: S.NullOr<S.Struct<{
        code: typeof S.String;
        message: typeof S.String;
        param: S.NullOr<typeof S.String>;
    }>>;
    fine_tuned_model: S.NullOr<typeof S.String>;
    finished_at: S.NullOr<typeof S.Int>;
    hyperparameters: S.Struct<{
        batch_size: S.optionalWith<S.Union<[typeof FineTuningJobHyperparametersBatchSizeEnum, S.filter<S.filter<typeof S.Int>>]>, {
            nullable: true;
            default: () => "auto";
        }>;
        learning_rate_multiplier: S.optionalWith<S.Union<[typeof FineTuningJobHyperparametersLearningRateMultiplierEnum, S.filter<typeof S.Number>]>, {
            nullable: true;
            default: () => "auto";
        }>;
        n_epochs: S.optionalWith<S.Union<[typeof FineTuningJobHyperparametersNEpochsEnum, S.filter<S.filter<typeof S.Int>>]>, {
            nullable: true;
            default: () => "auto";
        }>;
    }>;
    model: typeof S.String;
    object: typeof FineTuningJobObject;
    organization_id: typeof S.String;
    result_files: S.Array$<typeof S.String>;
    status: typeof FineTuningJobStatus;
    trained_tokens: S.NullOr<typeof S.Int>;
    training_file: typeof S.String;
    validation_file: S.NullOr<typeof S.String>;
    integrations: S.optionalWith<S.filter<S.Array$<typeof FineTuningIntegration>>, {
        nullable: true;
    }>;
    seed: typeof S.Int;
    estimated_finish: S.optionalWith<typeof S.Int, {
        nullable: true;
    }>;
    method: S.optionalWith<typeof FineTuneMethod, {
        nullable: true;
    }>;
    metadata: S.optionalWith<typeof Metadata, {
        nullable: true;
    }>;
}>;
export declare class FineTuningJob extends FineTuningJob_base {
}
declare const ListPaginatedFineTuningJobsResponseObject_base: S.Literal<["list"]>;
export declare class ListPaginatedFineTuningJobsResponseObject extends ListPaginatedFineTuningJobsResponseObject_base {
}
declare const ListPaginatedFineTuningJobsResponse_base: S.Class<ListPaginatedFineTuningJobsResponse, {
    data: S.Array$<typeof FineTuningJob>;
    has_more: typeof S.Boolean;
    object: typeof ListPaginatedFineTuningJobsResponseObject;
}, S.Struct.Encoded<{
    data: S.Array$<typeof FineTuningJob>;
    has_more: typeof S.Boolean;
    object: typeof ListPaginatedFineTuningJobsResponseObject;
}>, never, {
    readonly object: "list";
} & {
    readonly has_more: boolean;
} & {
    readonly data: readonly {
        readonly object: "fine_tuning.job";
        readonly model: string;
        readonly method?: {
            readonly type?: "supervised" | "dpo" | undefined;
            readonly supervised?: {
                readonly hyperparameters?: {
                    readonly batch_size: number | "auto";
                    readonly learning_rate_multiplier: number | "auto";
                    readonly n_epochs: number | "auto";
                } | undefined;
            } | undefined;
            readonly dpo?: {
                readonly hyperparameters?: {
                    readonly batch_size: number | "auto";
                    readonly learning_rate_multiplier: number | "auto";
                    readonly n_epochs: number | "auto";
                    readonly beta: number | "auto";
                } | undefined;
            } | undefined;
        } | undefined;
        readonly id: string;
        readonly created_at: number;
        readonly metadata?: {
            readonly [x: string]: unknown;
        } | undefined;
        readonly status: "failed" | "cancelled" | "validating_files" | "queued" | "running" | "succeeded";
        readonly seed: number;
        readonly error: {
            readonly message: string;
            readonly code: string;
            readonly param: string | null;
        } | null;
        readonly hyperparameters: {
            readonly batch_size: number | "auto";
            readonly learning_rate_multiplier: number | "auto";
            readonly n_epochs: number | "auto";
        };
        readonly fine_tuned_model: string | null;
        readonly finished_at: number | null;
        readonly organization_id: string;
        readonly result_files: readonly string[];
        readonly trained_tokens: number | null;
        readonly training_file: string;
        readonly validation_file: string | null;
        readonly integrations?: readonly {
            readonly type: "wandb";
            readonly wandb: {
                readonly name?: string | undefined;
                readonly project: string;
                readonly entity?: string | undefined;
                readonly tags?: readonly string[] | undefined;
            };
        }[] | undefined;
        readonly estimated_finish?: number | undefined;
    }[];
}, {}, {}>;
export declare class ListPaginatedFineTuningJobsResponse extends ListPaginatedFineTuningJobsResponse_base {
}
declare const CreateFineTuningJobRequestModelEnum_base: S.Literal<["babbage-002", "davinci-002", "gpt-3.5-turbo", "gpt-4o-mini"]>;
export declare class CreateFineTuningJobRequestModelEnum extends CreateFineTuningJobRequestModelEnum_base {
}
declare const CreateFineTuningJobRequestHyperparametersBatchSizeEnum_base: S.Literal<["auto"]>;
export declare class CreateFineTuningJobRequestHyperparametersBatchSizeEnum extends CreateFineTuningJobRequestHyperparametersBatchSizeEnum_base {
}
declare const CreateFineTuningJobRequestHyperparametersLearningRateMultiplierEnum_base: S.Literal<["auto"]>;
export declare class CreateFineTuningJobRequestHyperparametersLearningRateMultiplierEnum extends CreateFineTuningJobRequestHyperparametersLearningRateMultiplierEnum_base {
}
declare const CreateFineTuningJobRequestHyperparametersNEpochsEnum_base: S.Literal<["auto"]>;
export declare class CreateFineTuningJobRequestHyperparametersNEpochsEnum extends CreateFineTuningJobRequestHyperparametersNEpochsEnum_base {
}
declare const CreateFineTuningJobRequest_base: S.Class<CreateFineTuningJobRequest, {
    model: S.Union<[typeof S.String, typeof CreateFineTuningJobRequestModelEnum]>;
    training_file: typeof S.String;
    hyperparameters: S.optionalWith<S.Struct<{
        batch_size: S.optionalWith<S.Union<[typeof CreateFineTuningJobRequestHyperparametersBatchSizeEnum, S.filter<S.filter<typeof S.Int>>]>, {
            nullable: true;
            default: () => "auto";
        }>;
        learning_rate_multiplier: S.optionalWith<S.Union<[typeof CreateFineTuningJobRequestHyperparametersLearningRateMultiplierEnum, S.filter<typeof S.Number>]>, {
            nullable: true;
            default: () => "auto";
        }>;
        n_epochs: S.optionalWith<S.Union<[typeof CreateFineTuningJobRequestHyperparametersNEpochsEnum, S.filter<S.filter<typeof S.Int>>]>, {
            nullable: true;
            default: () => "auto";
        }>;
    }>, {
        nullable: true;
    }>;
    suffix: S.optionalWith<S.NullOr<S.filter<S.filter<typeof S.String>>>, {
        default: () => null;
    }>;
    validation_file: S.optionalWith<typeof S.String, {
        nullable: true;
    }>;
    integrations: S.optionalWith<S.Array$<S.Struct<{
        type: S.Literal<["wandb"]>;
        wandb: S.Struct<{
            project: typeof S.String;
            name: S.optionalWith<typeof S.String, {
                nullable: true;
            }>;
            entity: S.optionalWith<typeof S.String, {
                nullable: true;
            }>;
            tags: S.optionalWith<S.Array$<typeof S.String>, {
                nullable: true;
            }>;
        }>;
    }>>, {
        nullable: true;
    }>;
    seed: S.optionalWith<S.filter<S.filter<typeof S.Int>>, {
        nullable: true;
    }>;
    method: S.optionalWith<typeof FineTuneMethod, {
        nullable: true;
    }>;
    metadata: S.optionalWith<typeof Metadata, {
        nullable: true;
    }>;
}, S.Struct.Encoded<{
    model: S.Union<[typeof S.String, typeof CreateFineTuningJobRequestModelEnum]>;
    training_file: typeof S.String;
    hyperparameters: S.optionalWith<S.Struct<{
        batch_size: S.optionalWith<S.Union<[typeof CreateFineTuningJobRequestHyperparametersBatchSizeEnum, S.filter<S.filter<typeof S.Int>>]>, {
            nullable: true;
            default: () => "auto";
        }>;
        learning_rate_multiplier: S.optionalWith<S.Union<[typeof CreateFineTuningJobRequestHyperparametersLearningRateMultiplierEnum, S.filter<typeof S.Number>]>, {
            nullable: true;
            default: () => "auto";
        }>;
        n_epochs: S.optionalWith<S.Union<[typeof CreateFineTuningJobRequestHyperparametersNEpochsEnum, S.filter<S.filter<typeof S.Int>>]>, {
            nullable: true;
            default: () => "auto";
        }>;
    }>, {
        nullable: true;
    }>;
    suffix: S.optionalWith<S.NullOr<S.filter<S.filter<typeof S.String>>>, {
        default: () => null;
    }>;
    validation_file: S.optionalWith<typeof S.String, {
        nullable: true;
    }>;
    integrations: S.optionalWith<S.Array$<S.Struct<{
        type: S.Literal<["wandb"]>;
        wandb: S.Struct<{
            project: typeof S.String;
            name: S.optionalWith<typeof S.String, {
                nullable: true;
            }>;
            entity: S.optionalWith<typeof S.String, {
                nullable: true;
            }>;
            tags: S.optionalWith<S.Array$<typeof S.String>, {
                nullable: true;
            }>;
        }>;
    }>>, {
        nullable: true;
    }>;
    seed: S.optionalWith<S.filter<S.filter<typeof S.Int>>, {
        nullable: true;
    }>;
    method: S.optionalWith<typeof FineTuneMethod, {
        nullable: true;
    }>;
    metadata: S.optionalWith<typeof Metadata, {
        nullable: true;
    }>;
}>, never, {
    readonly model: string;
} & {
    readonly method?: {
        readonly type?: "supervised" | "dpo" | undefined;
        readonly supervised?: {
            readonly hyperparameters?: {
                readonly batch_size: number | "auto";
                readonly learning_rate_multiplier: number | "auto";
                readonly n_epochs: number | "auto";
            } | undefined;
        } | undefined;
        readonly dpo?: {
            readonly hyperparameters?: {
                readonly batch_size: number | "auto";
                readonly learning_rate_multiplier: number | "auto";
                readonly n_epochs: number | "auto";
                readonly beta: number | "auto";
            } | undefined;
        } | undefined;
    } | undefined;
} & {
    readonly metadata?: {
        readonly [x: string]: unknown;
    } | undefined;
} & {
    readonly seed?: number | undefined;
} & {
    readonly suffix?: string | null;
} & {
    readonly hyperparameters?: {
        readonly batch_size: number | "auto";
        readonly learning_rate_multiplier: number | "auto";
        readonly n_epochs: number | "auto";
    } | undefined;
} & {
    readonly training_file: string;
} & {
    readonly validation_file?: string | undefined;
} & {
    readonly integrations?: readonly {
        readonly type: "wandb";
        readonly wandb: {
            readonly name?: string | undefined;
            readonly project: string;
            readonly entity?: string | undefined;
            readonly tags?: readonly string[] | undefined;
        };
    }[] | undefined;
}, {}, {}>;
export declare class CreateFineTuningJobRequest extends CreateFineTuningJobRequest_base {
}
declare const ListFineTuningJobCheckpointsParams_base: S.Struct<{
    after: S.optionalWith<typeof S.String, {
        nullable: true;
    }>;
    limit: S.optionalWith<typeof S.Int, {
        nullable: true;
        default: () => 10;
    }>;
}>;
export declare class ListFineTuningJobCheckpointsParams extends ListFineTuningJobCheckpointsParams_base {
}
declare const FineTuningJobCheckpointObject_base: S.Literal<["fine_tuning.job.checkpoint"]>;
export declare class FineTuningJobCheckpointObject extends FineTuningJobCheckpointObject_base {
}
declare const FineTuningJobCheckpoint_base: S.Struct<{
    id: typeof S.String;
    created_at: typeof S.Int;
    fine_tuned_model_checkpoint: typeof S.String;
    step_number: typeof S.Int;
    metrics: S.Struct<{
        step: S.optionalWith<typeof S.Number, {
            nullable: true;
        }>;
        train_loss: S.optionalWith<typeof S.Number, {
            nullable: true;
        }>;
        train_mean_token_accuracy: S.optionalWith<typeof S.Number, {
            nullable: true;
        }>;
        valid_loss: S.optionalWith<typeof S.Number, {
            nullable: true;
        }>;
        valid_mean_token_accuracy: S.optionalWith<typeof S.Number, {
            nullable: true;
        }>;
        full_valid_loss: S.optionalWith<typeof S.Number, {
            nullable: true;
        }>;
        full_valid_mean_token_accuracy: S.optionalWith<typeof S.Number, {
            nullable: true;
        }>;
    }>;
    fine_tuning_job_id: typeof S.String;
    object: typeof FineTuningJobCheckpointObject;
}>;
export declare class FineTuningJobCheckpoint extends FineTuningJobCheckpoint_base {
}
declare const ListFineTuningJobCheckpointsResponseObject_base: S.Literal<["list"]>;
export declare class ListFineTuningJobCheckpointsResponseObject extends ListFineTuningJobCheckpointsResponseObject_base {
}
declare const ListFineTuningJobCheckpointsResponse_base: S.Class<ListFineTuningJobCheckpointsResponse, {
    data: S.Array$<typeof FineTuningJobCheckpoint>;
    object: typeof ListFineTuningJobCheckpointsResponseObject;
    first_id: S.optionalWith<typeof S.String, {
        nullable: true;
    }>;
    last_id: S.optionalWith<typeof S.String, {
        nullable: true;
    }>;
    has_more: typeof S.Boolean;
}, S.Struct.Encoded<{
    data: S.Array$<typeof FineTuningJobCheckpoint>;
    object: typeof ListFineTuningJobCheckpointsResponseObject;
    first_id: S.optionalWith<typeof S.String, {
        nullable: true;
    }>;
    last_id: S.optionalWith<typeof S.String, {
        nullable: true;
    }>;
    has_more: typeof S.Boolean;
}>, never, {
    readonly object: "list";
} & {
    readonly first_id?: string | undefined;
} & {
    readonly last_id?: string | undefined;
} & {
    readonly has_more: boolean;
} & {
    readonly data: readonly {
        readonly object: "fine_tuning.job.checkpoint";
        readonly id: string;
        readonly created_at: number;
        readonly fine_tuned_model_checkpoint: string;
        readonly step_number: number;
        readonly metrics: {
            readonly step?: number | undefined;
            readonly train_loss?: number | undefined;
            readonly train_mean_token_accuracy?: number | undefined;
            readonly valid_loss?: number | undefined;
            readonly valid_mean_token_accuracy?: number | undefined;
            readonly full_valid_loss?: number | undefined;
            readonly full_valid_mean_token_accuracy?: number | undefined;
        };
        readonly fine_tuning_job_id: string;
    }[];
}, {}, {}>;
export declare class ListFineTuningJobCheckpointsResponse extends ListFineTuningJobCheckpointsResponse_base {
}
declare const ListFineTuningEventsParams_base: S.Struct<{
    after: S.optionalWith<typeof S.String, {
        nullable: true;
    }>;
    limit: S.optionalWith<typeof S.Int, {
        nullable: true;
        default: () => 20;
    }>;
}>;
export declare class ListFineTuningEventsParams extends ListFineTuningEventsParams_base {
}
declare const FineTuningJobEventObject_base: S.Literal<["fine_tuning.job.event"]>;
export declare class FineTuningJobEventObject extends FineTuningJobEventObject_base {
}
declare const FineTuningJobEventLevel_base: S.Literal<["info", "warn", "error"]>;
export declare class FineTuningJobEventLevel extends FineTuningJobEventLevel_base {
}
declare const FineTuningJobEventType_base: S.Literal<["message", "metrics"]>;
export declare class FineTuningJobEventType extends FineTuningJobEventType_base {
}
declare const FineTuningJobEvent_base: S.Struct<{
    object: typeof FineTuningJobEventObject;
    id: typeof S.String;
    created_at: typeof S.Int;
    level: typeof FineTuningJobEventLevel;
    message: typeof S.String;
    type: S.optionalWith<typeof FineTuningJobEventType, {
        nullable: true;
    }>;
    data: S.optionalWith<S.Record$<typeof S.String, typeof S.Unknown>, {
        nullable: true;
    }>;
}>;
export declare class FineTuningJobEvent extends FineTuningJobEvent_base {
}
declare const ListFineTuningJobEventsResponseObject_base: S.Literal<["list"]>;
export declare class ListFineTuningJobEventsResponseObject extends ListFineTuningJobEventsResponseObject_base {
}
declare const ListFineTuningJobEventsResponse_base: S.Class<ListFineTuningJobEventsResponse, {
    data: S.Array$<typeof FineTuningJobEvent>;
    object: typeof ListFineTuningJobEventsResponseObject;
    has_more: typeof S.Boolean;
}, S.Struct.Encoded<{
    data: S.Array$<typeof FineTuningJobEvent>;
    object: typeof ListFineTuningJobEventsResponseObject;
    has_more: typeof S.Boolean;
}>, never, {
    readonly object: "list";
} & {
    readonly has_more: boolean;
} & {
    readonly data: readonly {
        readonly object: "fine_tuning.job.event";
        readonly message: string;
        readonly type?: "message" | "metrics" | undefined;
        readonly id: string;
        readonly created_at: number;
        readonly data?: {
            readonly [x: string]: unknown;
        } | undefined;
        readonly level: "error" | "info" | "warn";
    }[];
}, {}, {}>;
export declare class ListFineTuningJobEventsResponse extends ListFineTuningJobEventsResponse_base {
}
declare const Image_base: S.Struct<{
    b64_json: S.optionalWith<typeof S.String, {
        nullable: true;
    }>;
    url: S.optionalWith<typeof S.String, {
        nullable: true;
    }>;
    revised_prompt: S.optionalWith<typeof S.String, {
        nullable: true;
    }>;
}>;
export declare class Image extends Image_base {
}
declare const ImagesResponse_base: S.Class<ImagesResponse, {
    created: typeof S.Int;
    data: S.Array$<typeof Image>;
}, S.Struct.Encoded<{
    created: typeof S.Int;
    data: S.Array$<typeof Image>;
}>, never, {
    readonly data: readonly {
        readonly url?: string | undefined;
        readonly b64_json?: string | undefined;
        readonly revised_prompt?: string | undefined;
    }[];
} & {
    readonly created: number;
}, {}, {}>;
export declare class ImagesResponse extends ImagesResponse_base {
}
declare const CreateImageRequestModelEnum_base: S.Literal<["dall-e-2", "dall-e-3"]>;
export declare class CreateImageRequestModelEnum extends CreateImageRequestModelEnum_base {
}
declare const CreateImageRequestQuality_base: S.Literal<["standard", "hd"]>;
export declare class CreateImageRequestQuality extends CreateImageRequestQuality_base {
}
declare const CreateImageRequestResponseFormat_base: S.Literal<["url", "b64_json"]>;
export declare class CreateImageRequestResponseFormat extends CreateImageRequestResponseFormat_base {
}
declare const CreateImageRequestSize_base: S.Literal<["256x256", "512x512", "1024x1024", "1792x1024", "1024x1792"]>;
export declare class CreateImageRequestSize extends CreateImageRequestSize_base {
}
declare const CreateImageRequestStyle_base: S.Literal<["vivid", "natural"]>;
export declare class CreateImageRequestStyle extends CreateImageRequestStyle_base {
}
declare const CreateImageRequest_base: S.Class<CreateImageRequest, {
    prompt: typeof S.String;
    model: S.optionalWith<S.Union<[typeof S.String, typeof CreateImageRequestModelEnum]>, {
        nullable: true;
        default: () => "dall-e-2";
    }>;
    n: S.optionalWith<S.filter<S.filter<typeof S.Int>>, {
        nullable: true;
        default: () => 1;
    }>;
    quality: S.optionalWith<typeof CreateImageRequestQuality, {
        nullable: true;
        default: () => "standard";
    }>;
    response_format: S.optionalWith<typeof CreateImageRequestResponseFormat, {
        nullable: true;
        default: () => "url";
    }>;
    size: S.optionalWith<typeof CreateImageRequestSize, {
        nullable: true;
        default: () => "1024x1024";
    }>;
    style: S.optionalWith<typeof CreateImageRequestStyle, {
        nullable: true;
        default: () => "vivid";
    }>;
    user: S.optionalWith<typeof S.String, {
        nullable: true;
    }>;
}, S.Struct.Encoded<{
    prompt: typeof S.String;
    model: S.optionalWith<S.Union<[typeof S.String, typeof CreateImageRequestModelEnum]>, {
        nullable: true;
        default: () => "dall-e-2";
    }>;
    n: S.optionalWith<S.filter<S.filter<typeof S.Int>>, {
        nullable: true;
        default: () => 1;
    }>;
    quality: S.optionalWith<typeof CreateImageRequestQuality, {
        nullable: true;
        default: () => "standard";
    }>;
    response_format: S.optionalWith<typeof CreateImageRequestResponseFormat, {
        nullable: true;
        default: () => "url";
    }>;
    size: S.optionalWith<typeof CreateImageRequestSize, {
        nullable: true;
        default: () => "1024x1024";
    }>;
    style: S.optionalWith<typeof CreateImageRequestStyle, {
        nullable: true;
        default: () => "vivid";
    }>;
    user: S.optionalWith<typeof S.String, {
        nullable: true;
    }>;
}>, never, {
    readonly model?: string;
} & {
    readonly response_format?: "url" | "b64_json";
} & {
    readonly user?: string | undefined;
} & {
    readonly n?: number;
} & {
    readonly prompt: string;
} & {
    readonly quality?: "standard" | "hd";
} & {
    readonly size?: "256x256" | "512x512" | "1024x1024" | "1792x1024" | "1024x1792";
} & {
    readonly style?: "vivid" | "natural";
}, {}, {}>;
export declare class CreateImageRequest extends CreateImageRequest_base {
}
declare const ListModelsResponseObject_base: S.Literal<["list"]>;
export declare class ListModelsResponseObject extends ListModelsResponseObject_base {
}
declare const ModelObject_base: S.Literal<["model"]>;
export declare class ModelObject extends ModelObject_base {
}
declare const Model_base: S.Struct<{
    id: typeof S.String;
    created: typeof S.Int;
    object: typeof ModelObject;
    owned_by: typeof S.String;
}>;
export declare class Model extends Model_base {
}
declare const ListModelsResponse_base: S.Class<ListModelsResponse, {
    object: typeof ListModelsResponseObject;
    data: S.Array$<typeof Model>;
}, S.Struct.Encoded<{
    object: typeof ListModelsResponseObject;
    data: S.Array$<typeof Model>;
}>, never, {
    readonly object: "list";
} & {
    readonly data: readonly {
        readonly object: "model";
        readonly id: string;
        readonly created: number;
        readonly owned_by: string;
    }[];
}, {}, {}>;
export declare class ListModelsResponse extends ListModelsResponse_base {
}
declare const DeleteModelResponse_base: S.Class<DeleteModelResponse, {
    id: typeof S.String;
    deleted: typeof S.Boolean;
    object: typeof S.String;
}, S.Struct.Encoded<{
    id: typeof S.String;
    deleted: typeof S.Boolean;
    object: typeof S.String;
}>, never, {
    readonly object: string;
} & {
    readonly id: string;
} & {
    readonly deleted: boolean;
}, {}, {}>;
export declare class DeleteModelResponse extends DeleteModelResponse_base {
}
declare const CreateModerationRequestModelEnum_base: S.Literal<["omni-moderation-latest", "omni-moderation-2024-09-26", "text-moderation-latest", "text-moderation-stable"]>;
export declare class CreateModerationRequestModelEnum extends CreateModerationRequestModelEnum_base {
}
declare const CreateModerationRequest_base: S.Class<CreateModerationRequest, {
    input: S.Union<[typeof S.String, S.Array$<typeof S.String>, S.Array$<S.Union<[S.Struct<{
        type: S.Literal<["image_url"]>;
        image_url: S.Struct<{
            url: typeof S.String;
        }>;
    }>, S.Struct<{
        type: S.Literal<["text"]>;
        text: typeof S.String;
    }>]>>]>;
    model: S.optionalWith<S.Union<[typeof S.String, typeof CreateModerationRequestModelEnum]>, {
        nullable: true;
        default: () => "omni-moderation-latest";
    }>;
}, S.Struct.Encoded<{
    input: S.Union<[typeof S.String, S.Array$<typeof S.String>, S.Array$<S.Union<[S.Struct<{
        type: S.Literal<["image_url"]>;
        image_url: S.Struct<{
            url: typeof S.String;
        }>;
    }>, S.Struct<{
        type: S.Literal<["text"]>;
        text: typeof S.String;
    }>]>>]>;
    model: S.optionalWith<S.Union<[typeof S.String, typeof CreateModerationRequestModelEnum]>, {
        nullable: true;
        default: () => "omni-moderation-latest";
    }>;
}>, never, {
    readonly model?: string;
} & {
    readonly input: string | readonly string[] | readonly ({
        readonly type: "image_url";
        readonly image_url: {
            readonly url: string;
        };
    } | {
        readonly type: "text";
        readonly text: string;
    })[];
}, {}, {}>;
export declare class CreateModerationRequest extends CreateModerationRequest_base {
}
declare const CreateModerationResponse_base: S.Class<CreateModerationResponse, {
    id: typeof S.String;
    model: typeof S.String;
    results: S.Array$<S.Struct<{
        flagged: typeof S.Boolean;
        categories: S.Struct<{
            hate: typeof S.Boolean;
            "hate/threatening": typeof S.Boolean;
            harassment: typeof S.Boolean;
            "harassment/threatening": typeof S.Boolean;
            illicit: S.NullOr<typeof S.Boolean>;
            "illicit/violent": S.NullOr<typeof S.Boolean>;
            "self-harm": typeof S.Boolean;
            "self-harm/intent": typeof S.Boolean;
            "self-harm/instructions": typeof S.Boolean;
            sexual: typeof S.Boolean;
            "sexual/minors": typeof S.Boolean;
            violence: typeof S.Boolean;
            "violence/graphic": typeof S.Boolean;
        }>;
        category_scores: S.Struct<{
            hate: typeof S.Number;
            "hate/threatening": typeof S.Number;
            harassment: typeof S.Number;
            "harassment/threatening": typeof S.Number;
            illicit: typeof S.Number;
            "illicit/violent": typeof S.Number;
            "self-harm": typeof S.Number;
            "self-harm/intent": typeof S.Number;
            "self-harm/instructions": typeof S.Number;
            sexual: typeof S.Number;
            "sexual/minors": typeof S.Number;
            violence: typeof S.Number;
            "violence/graphic": typeof S.Number;
        }>;
        category_applied_input_types: S.Struct<{
            hate: S.Array$<S.Literal<["text"]>>;
            "hate/threatening": S.Array$<S.Literal<["text"]>>;
            harassment: S.Array$<S.Literal<["text"]>>;
            "harassment/threatening": S.Array$<S.Literal<["text"]>>;
            illicit: S.Array$<S.Literal<["text"]>>;
            "illicit/violent": S.Array$<S.Literal<["text"]>>;
            "self-harm": S.Array$<S.Literal<["text", "image"]>>;
            "self-harm/intent": S.Array$<S.Literal<["text", "image"]>>;
            "self-harm/instructions": S.Array$<S.Literal<["text", "image"]>>;
            sexual: S.Array$<S.Literal<["text", "image"]>>;
            "sexual/minors": S.Array$<S.Literal<["text"]>>;
            violence: S.Array$<S.Literal<["text", "image"]>>;
            "violence/graphic": S.Array$<S.Literal<["text", "image"]>>;
        }>;
    }>>;
}, S.Struct.Encoded<{
    id: typeof S.String;
    model: typeof S.String;
    results: S.Array$<S.Struct<{
        flagged: typeof S.Boolean;
        categories: S.Struct<{
            hate: typeof S.Boolean;
            "hate/threatening": typeof S.Boolean;
            harassment: typeof S.Boolean;
            "harassment/threatening": typeof S.Boolean;
            illicit: S.NullOr<typeof S.Boolean>;
            "illicit/violent": S.NullOr<typeof S.Boolean>;
            "self-harm": typeof S.Boolean;
            "self-harm/intent": typeof S.Boolean;
            "self-harm/instructions": typeof S.Boolean;
            sexual: typeof S.Boolean;
            "sexual/minors": typeof S.Boolean;
            violence: typeof S.Boolean;
            "violence/graphic": typeof S.Boolean;
        }>;
        category_scores: S.Struct<{
            hate: typeof S.Number;
            "hate/threatening": typeof S.Number;
            harassment: typeof S.Number;
            "harassment/threatening": typeof S.Number;
            illicit: typeof S.Number;
            "illicit/violent": typeof S.Number;
            "self-harm": typeof S.Number;
            "self-harm/intent": typeof S.Number;
            "self-harm/instructions": typeof S.Number;
            sexual: typeof S.Number;
            "sexual/minors": typeof S.Number;
            violence: typeof S.Number;
            "violence/graphic": typeof S.Number;
        }>;
        category_applied_input_types: S.Struct<{
            hate: S.Array$<S.Literal<["text"]>>;
            "hate/threatening": S.Array$<S.Literal<["text"]>>;
            harassment: S.Array$<S.Literal<["text"]>>;
            "harassment/threatening": S.Array$<S.Literal<["text"]>>;
            illicit: S.Array$<S.Literal<["text"]>>;
            "illicit/violent": S.Array$<S.Literal<["text"]>>;
            "self-harm": S.Array$<S.Literal<["text", "image"]>>;
            "self-harm/intent": S.Array$<S.Literal<["text", "image"]>>;
            "self-harm/instructions": S.Array$<S.Literal<["text", "image"]>>;
            sexual: S.Array$<S.Literal<["text", "image"]>>;
            "sexual/minors": S.Array$<S.Literal<["text"]>>;
            violence: S.Array$<S.Literal<["text", "image"]>>;
            "violence/graphic": S.Array$<S.Literal<["text", "image"]>>;
        }>;
    }>>;
}>, never, {
    readonly model: string;
} & {
    readonly id: string;
} & {
    readonly results: readonly {
        readonly flagged: boolean;
        readonly categories: {
            readonly hate: boolean;
            readonly "hate/threatening": boolean;
            readonly harassment: boolean;
            readonly "harassment/threatening": boolean;
            readonly illicit: boolean | null;
            readonly "illicit/violent": boolean | null;
            readonly "self-harm": boolean;
            readonly "self-harm/intent": boolean;
            readonly "self-harm/instructions": boolean;
            readonly sexual: boolean;
            readonly "sexual/minors": boolean;
            readonly violence: boolean;
            readonly "violence/graphic": boolean;
        };
        readonly category_scores: {
            readonly hate: number;
            readonly "hate/threatening": number;
            readonly harassment: number;
            readonly "harassment/threatening": number;
            readonly illicit: number;
            readonly "illicit/violent": number;
            readonly "self-harm": number;
            readonly "self-harm/intent": number;
            readonly "self-harm/instructions": number;
            readonly sexual: number;
            readonly "sexual/minors": number;
            readonly violence: number;
            readonly "violence/graphic": number;
        };
        readonly category_applied_input_types: {
            readonly hate: readonly "text"[];
            readonly "hate/threatening": readonly "text"[];
            readonly harassment: readonly "text"[];
            readonly "harassment/threatening": readonly "text"[];
            readonly illicit: readonly "text"[];
            readonly "illicit/violent": readonly "text"[];
            readonly "self-harm": readonly ("text" | "image")[];
            readonly "self-harm/intent": readonly ("text" | "image")[];
            readonly "self-harm/instructions": readonly ("text" | "image")[];
            readonly sexual: readonly ("text" | "image")[];
            readonly "sexual/minors": readonly "text"[];
            readonly violence: readonly ("text" | "image")[];
            readonly "violence/graphic": readonly ("text" | "image")[];
        };
    }[];
}, {}, {}>;
export declare class CreateModerationResponse extends CreateModerationResponse_base {
}
declare const AdminApiKeysListParamsOrder_base: S.Literal<["asc", "desc"]>;
export declare class AdminApiKeysListParamsOrder extends AdminApiKeysListParamsOrder_base {
}
declare const AdminApiKeysListParams_base: S.Struct<{
    after: S.optionalWith<typeof S.String, {
        nullable: true;
    }>;
    order: S.optionalWith<typeof AdminApiKeysListParamsOrder, {
        nullable: true;
        default: () => "asc";
    }>;
    limit: S.optionalWith<typeof S.Int, {
        nullable: true;
        default: () => 20;
    }>;
}>;
export declare class AdminApiKeysListParams extends AdminApiKeysListParams_base {
}
declare const AdminApiKey_base: S.Struct<{
    object: S.optionalWith<typeof S.String, {
        nullable: true;
    }>;
    id: S.optionalWith<typeof S.String, {
        nullable: true;
    }>;
    name: S.optionalWith<typeof S.String, {
        nullable: true;
    }>;
    redacted_value: S.optionalWith<typeof S.String, {
        nullable: true;
    }>;
    value: S.optionalWith<typeof S.String, {
        nullable: true;
    }>;
    created_at: S.optionalWith<typeof S.Int, {
        nullable: true;
    }>;
    owner: S.optionalWith<S.Struct<{
        type: S.optionalWith<typeof S.String, {
            nullable: true;
        }>;
        id: S.optionalWith<typeof S.String, {
            nullable: true;
        }>;
        name: S.optionalWith<typeof S.String, {
            nullable: true;
        }>;
        created_at: S.optionalWith<typeof S.Int, {
            nullable: true;
        }>;
        role: S.optionalWith<typeof S.String, {
            nullable: true;
        }>;
    }>, {
        nullable: true;
    }>;
}>;
export declare class AdminApiKey extends AdminApiKey_base {
}
declare const ApiKeyList_base: S.Class<ApiKeyList, {
    object: S.optionalWith<typeof S.String, {
        nullable: true;
    }>;
    data: S.optionalWith<S.Array$<typeof AdminApiKey>, {
        nullable: true;
    }>;
    has_more: S.optionalWith<typeof S.Boolean, {
        nullable: true;
    }>;
    first_id: S.optionalWith<typeof S.String, {
        nullable: true;
    }>;
    last_id: S.optionalWith<typeof S.String, {
        nullable: true;
    }>;
}, S.Struct.Encoded<{
    object: S.optionalWith<typeof S.String, {
        nullable: true;
    }>;
    data: S.optionalWith<S.Array$<typeof AdminApiKey>, {
        nullable: true;
    }>;
    has_more: S.optionalWith<typeof S.Boolean, {
        nullable: true;
    }>;
    first_id: S.optionalWith<typeof S.String, {
        nullable: true;
    }>;
    last_id: S.optionalWith<typeof S.String, {
        nullable: true;
    }>;
}>, never, {
    readonly object?: string | undefined;
} & {
    readonly first_id?: string | undefined;
} & {
    readonly last_id?: string | undefined;
} & {
    readonly has_more?: boolean | undefined;
} & {
    readonly data?: readonly {
        readonly object?: string | undefined;
        readonly value?: string | undefined;
        readonly name?: string | undefined;
        readonly id?: string | undefined;
        readonly created_at?: number | undefined;
        readonly redacted_value?: string | undefined;
        readonly owner?: {
            readonly role?: string | undefined;
            readonly type?: string | undefined;
            readonly name?: string | undefined;
            readonly id?: string | undefined;
            readonly created_at?: number | undefined;
        } | undefined;
    }[] | undefined;
}, {}, {}>;
export declare class ApiKeyList extends ApiKeyList_base {
}
declare const AdminApiKeysCreateRequest_base: S.Class<AdminApiKeysCreateRequest, {
    name: typeof S.String;
}, S.Struct.Encoded<{
    name: typeof S.String;
}>, never, {
    readonly name: string;
}, {}, {}>;
export declare class AdminApiKeysCreateRequest extends AdminApiKeysCreateRequest_base {
}
declare const AdminApiKeysDelete200_base: S.Struct<{
    id: S.optionalWith<typeof S.String, {
        nullable: true;
    }>;
    object: S.optionalWith<typeof S.String, {
        nullable: true;
    }>;
    deleted: S.optionalWith<typeof S.Boolean, {
        nullable: true;
    }>;
}>;
export declare class AdminApiKeysDelete200 extends AdminApiKeysDelete200_base {
}
declare const AuditLogEventType_base: S.Literal<["api_key.created", "api_key.updated", "api_key.deleted", "invite.sent", "invite.accepted", "invite.deleted", "login.succeeded", "login.failed", "logout.succeeded", "logout.failed", "organization.updated", "project.created", "project.updated", "project.archived", "service_account.created", "service_account.updated", "service_account.deleted", "rate_limit.updated", "rate_limit.deleted", "user.added", "user.updated", "user.deleted"]>;
export declare class AuditLogEventType extends AuditLogEventType_base {
}
declare const ListAuditLogsParams_base: S.Struct<{
    "effective_at[gt]": S.optionalWith<typeof S.Int, {
        nullable: true;
    }>;
    "effective_at[gte]": S.optionalWith<typeof S.Int, {
        nullable: true;
    }>;
    "effective_at[lt]": S.optionalWith<typeof S.Int, {
        nullable: true;
    }>;
    "effective_at[lte]": S.optionalWith<typeof S.Int, {
        nullable: true;
    }>;
    "project_ids[]": S.optionalWith<S.Array$<typeof S.String>, {
        nullable: true;
    }>;
    "event_types[]": S.optionalWith<S.Array$<typeof AuditLogEventType>, {
        nullable: true;
    }>;
    "actor_ids[]": S.optionalWith<S.Array$<typeof S.String>, {
        nullable: true;
    }>;
    "actor_emails[]": S.optionalWith<S.Array$<typeof S.String>, {
        nullable: true;
    }>;
    "resource_ids[]": S.optionalWith<S.Array$<typeof S.String>, {
        nullable: true;
    }>;
    limit: S.optionalWith<typeof S.Int, {
        nullable: true;
        default: () => 20;
    }>;
    after: S.optionalWith<typeof S.String, {
        nullable: true;
    }>;
    before: S.optionalWith<typeof S.String, {
        nullable: true;
    }>;
}>;
export declare class ListAuditLogsParams extends ListAuditLogsParams_base {
}
declare const ListAuditLogsResponseObject_base: S.Literal<["list"]>;
export declare class ListAuditLogsResponseObject extends ListAuditLogsResponseObject_base {
}
declare const AuditLogActorType_base: S.Literal<["session", "api_key"]>;
export declare class AuditLogActorType extends AuditLogActorType_base {
}
declare const AuditLogActorUser_base: S.Struct<{
    id: S.optionalWith<typeof S.String, {
        nullable: true;
    }>;
    email: S.optionalWith<typeof S.String, {
        nullable: true;
    }>;
}>;
export declare class AuditLogActorUser extends AuditLogActorUser_base {
}
declare const AuditLogActorSession_base: S.Struct<{
    user: S.optionalWith<typeof AuditLogActorUser, {
        nullable: true;
    }>;
    ip_address: S.optionalWith<typeof S.String, {
        nullable: true;
    }>;
}>;
export declare class AuditLogActorSession extends AuditLogActorSession_base {
}
declare const AuditLogActorApiKeyType_base: S.Literal<["user", "service_account"]>;
export declare class AuditLogActorApiKeyType extends AuditLogActorApiKeyType_base {
}
declare const AuditLogActorServiceAccount_base: S.Struct<{
    id: S.optionalWith<typeof S.String, {
        nullable: true;
    }>;
}>;
export declare class AuditLogActorServiceAccount extends AuditLogActorServiceAccount_base {
}
declare const AuditLogActorApiKey_base: S.Struct<{
    id: S.optionalWith<typeof S.String, {
        nullable: true;
    }>;
    type: S.optionalWith<typeof AuditLogActorApiKeyType, {
        nullable: true;
    }>;
    user: S.optionalWith<typeof AuditLogActorUser, {
        nullable: true;
    }>;
    service_account: S.optionalWith<typeof AuditLogActorServiceAccount, {
        nullable: true;
    }>;
}>;
export declare class AuditLogActorApiKey extends AuditLogActorApiKey_base {
}
declare const AuditLogActor_base: S.Struct<{
    type: S.optionalWith<typeof AuditLogActorType, {
        nullable: true;
    }>;
    session: S.optionalWith<typeof AuditLogActorSession, {
        nullable: true;
    }>;
    api_key: S.optionalWith<typeof AuditLogActorApiKey, {
        nullable: true;
    }>;
}>;
export declare class AuditLogActor extends AuditLogActor_base {
}
declare const AuditLog_base: S.Struct<{
    id: typeof S.String;
    type: typeof AuditLogEventType;
    effective_at: typeof S.Int;
    project: S.optionalWith<S.Struct<{
        id: S.optionalWith<typeof S.String, {
            nullable: true;
        }>;
        name: S.optionalWith<typeof S.String, {
            nullable: true;
        }>;
    }>, {
        nullable: true;
    }>;
    actor: typeof AuditLogActor;
    "api_key.created": S.optionalWith<S.Struct<{
        id: S.optionalWith<typeof S.String, {
            nullable: true;
        }>;
        data: S.optionalWith<S.Struct<{
            scopes: S.optionalWith<S.Array$<typeof S.String>, {
                nullable: true;
            }>;
        }>, {
            nullable: true;
        }>;
    }>, {
        nullable: true;
    }>;
    "api_key.updated": S.optionalWith<S.Struct<{
        id: S.optionalWith<typeof S.String, {
            nullable: true;
        }>;
        changes_requested: S.optionalWith<S.Struct<{
            scopes: S.optionalWith<S.Array$<typeof S.String>, {
                nullable: true;
            }>;
        }>, {
            nullable: true;
        }>;
    }>, {
        nullable: true;
    }>;
    "api_key.deleted": S.optionalWith<S.Struct<{
        id: S.optionalWith<typeof S.String, {
            nullable: true;
        }>;
    }>, {
        nullable: true;
    }>;
    "invite.sent": S.optionalWith<S.Struct<{
        id: S.optionalWith<typeof S.String, {
            nullable: true;
        }>;
        data: S.optionalWith<S.Struct<{
            email: S.optionalWith<typeof S.String, {
                nullable: true;
            }>;
            role: S.optionalWith<typeof S.String, {
                nullable: true;
            }>;
        }>, {
            nullable: true;
        }>;
    }>, {
        nullable: true;
    }>;
    "invite.accepted": S.optionalWith<S.Struct<{
        id: S.optionalWith<typeof S.String, {
            nullable: true;
        }>;
    }>, {
        nullable: true;
    }>;
    "invite.deleted": S.optionalWith<S.Struct<{
        id: S.optionalWith<typeof S.String, {
            nullable: true;
        }>;
    }>, {
        nullable: true;
    }>;
    "login.failed": S.optionalWith<S.Struct<{
        error_code: S.optionalWith<typeof S.String, {
            nullable: true;
        }>;
        error_message: S.optionalWith<typeof S.String, {
            nullable: true;
        }>;
    }>, {
        nullable: true;
    }>;
    "logout.failed": S.optionalWith<S.Struct<{
        error_code: S.optionalWith<typeof S.String, {
            nullable: true;
        }>;
        error_message: S.optionalWith<typeof S.String, {
            nullable: true;
        }>;
    }>, {
        nullable: true;
    }>;
    "organization.updated": S.optionalWith<S.Struct<{
        id: S.optionalWith<typeof S.String, {
            nullable: true;
        }>;
        changes_requested: S.optionalWith<S.Struct<{
            title: S.optionalWith<typeof S.String, {
                nullable: true;
            }>;
            description: S.optionalWith<typeof S.String, {
                nullable: true;
            }>;
            name: S.optionalWith<typeof S.String, {
                nullable: true;
            }>;
            settings: S.optionalWith<S.Struct<{
                threads_ui_visibility: S.optionalWith<typeof S.String, {
                    nullable: true;
                }>;
                usage_dashboard_visibility: S.optionalWith<typeof S.String, {
                    nullable: true;
                }>;
            }>, {
                nullable: true;
            }>;
        }>, {
            nullable: true;
        }>;
    }>, {
        nullable: true;
    }>;
    "project.created": S.optionalWith<S.Struct<{
        id: S.optionalWith<typeof S.String, {
            nullable: true;
        }>;
        data: S.optionalWith<S.Struct<{
            name: S.optionalWith<typeof S.String, {
                nullable: true;
            }>;
            title: S.optionalWith<typeof S.String, {
                nullable: true;
            }>;
        }>, {
            nullable: true;
        }>;
    }>, {
        nullable: true;
    }>;
    "project.updated": S.optionalWith<S.Struct<{
        id: S.optionalWith<typeof S.String, {
            nullable: true;
        }>;
        changes_requested: S.optionalWith<S.Struct<{
            title: S.optionalWith<typeof S.String, {
                nullable: true;
            }>;
        }>, {
            nullable: true;
        }>;
    }>, {
        nullable: true;
    }>;
    "project.archived": S.optionalWith<S.Struct<{
        id: S.optionalWith<typeof S.String, {
            nullable: true;
        }>;
    }>, {
        nullable: true;
    }>;
    "rate_limit.updated": S.optionalWith<S.Struct<{
        id: S.optionalWith<typeof S.String, {
            nullable: true;
        }>;
        changes_requested: S.optionalWith<S.Struct<{
            max_requests_per_1_minute: S.optionalWith<typeof S.Int, {
                nullable: true;
            }>;
            max_tokens_per_1_minute: S.optionalWith<typeof S.Int, {
                nullable: true;
            }>;
            max_images_per_1_minute: S.optionalWith<typeof S.Int, {
                nullable: true;
            }>;
            max_audio_megabytes_per_1_minute: S.optionalWith<typeof S.Int, {
                nullable: true;
            }>;
            max_requests_per_1_day: S.optionalWith<typeof S.Int, {
                nullable: true;
            }>;
            batch_1_day_max_input_tokens: S.optionalWith<typeof S.Int, {
                nullable: true;
            }>;
        }>, {
            nullable: true;
        }>;
    }>, {
        nullable: true;
    }>;
    "rate_limit.deleted": S.optionalWith<S.Struct<{
        id: S.optionalWith<typeof S.String, {
            nullable: true;
        }>;
    }>, {
        nullable: true;
    }>;
    "service_account.created": S.optionalWith<S.Struct<{
        id: S.optionalWith<typeof S.String, {
            nullable: true;
        }>;
        data: S.optionalWith<S.Struct<{
            role: S.optionalWith<typeof S.String, {
                nullable: true;
            }>;
        }>, {
            nullable: true;
        }>;
    }>, {
        nullable: true;
    }>;
    "service_account.updated": S.optionalWith<S.Struct<{
        id: S.optionalWith<typeof S.String, {
            nullable: true;
        }>;
        changes_requested: S.optionalWith<S.Struct<{
            role: S.optionalWith<typeof S.String, {
                nullable: true;
            }>;
        }>, {
            nullable: true;
        }>;
    }>, {
        nullable: true;
    }>;
    "service_account.deleted": S.optionalWith<S.Struct<{
        id: S.optionalWith<typeof S.String, {
            nullable: true;
        }>;
    }>, {
        nullable: true;
    }>;
    "user.added": S.optionalWith<S.Struct<{
        id: S.optionalWith<typeof S.String, {
            nullable: true;
        }>;
        data: S.optionalWith<S.Struct<{
            role: S.optionalWith<typeof S.String, {
                nullable: true;
            }>;
        }>, {
            nullable: true;
        }>;
    }>, {
        nullable: true;
    }>;
    "user.updated": S.optionalWith<S.Struct<{
        id: S.optionalWith<typeof S.String, {
            nullable: true;
        }>;
        changes_requested: S.optionalWith<S.Struct<{
            role: S.optionalWith<typeof S.String, {
                nullable: true;
            }>;
        }>, {
            nullable: true;
        }>;
    }>, {
        nullable: true;
    }>;
    "user.deleted": S.optionalWith<S.Struct<{
        id: S.optionalWith<typeof S.String, {
            nullable: true;
        }>;
    }>, {
        nullable: true;
    }>;
}>;
export declare class AuditLog extends AuditLog_base {
}
declare const ListAuditLogsResponse_base: S.Class<ListAuditLogsResponse, {
    object: typeof ListAuditLogsResponseObject;
    data: S.Array$<typeof AuditLog>;
    first_id: typeof S.String;
    last_id: typeof S.String;
    has_more: typeof S.Boolean;
}, S.Struct.Encoded<{
    object: typeof ListAuditLogsResponseObject;
    data: S.Array$<typeof AuditLog>;
    first_id: typeof S.String;
    last_id: typeof S.String;
    has_more: typeof S.Boolean;
}>, never, {
    readonly object: "list";
} & {
    readonly first_id: string;
} & {
    readonly last_id: string;
} & {
    readonly has_more: boolean;
} & {
    readonly data: readonly {
        readonly type: "api_key.created" | "api_key.updated" | "api_key.deleted" | "invite.sent" | "invite.accepted" | "invite.deleted" | "login.succeeded" | "login.failed" | "logout.succeeded" | "logout.failed" | "organization.updated" | "project.created" | "project.updated" | "project.archived" | "service_account.created" | "service_account.updated" | "service_account.deleted" | "rate_limit.updated" | "rate_limit.deleted" | "user.added" | "user.updated" | "user.deleted";
        readonly id: string;
        readonly project?: {
            readonly name?: string | undefined;
            readonly id?: string | undefined;
        } | undefined;
        readonly "api_key.created"?: {
            readonly id?: string | undefined;
            readonly data?: {
                readonly scopes?: readonly string[] | undefined;
            } | undefined;
        } | undefined;
        readonly "api_key.updated"?: {
            readonly id?: string | undefined;
            readonly changes_requested?: {
                readonly scopes?: readonly string[] | undefined;
            } | undefined;
        } | undefined;
        readonly "api_key.deleted"?: {
            readonly id?: string | undefined;
        } | undefined;
        readonly "invite.sent"?: {
            readonly id?: string | undefined;
            readonly data?: {
                readonly role?: string | undefined;
                readonly email?: string | undefined;
            } | undefined;
        } | undefined;
        readonly "invite.accepted"?: {
            readonly id?: string | undefined;
        } | undefined;
        readonly "invite.deleted"?: {
            readonly id?: string | undefined;
        } | undefined;
        readonly "login.failed"?: {
            readonly error_code?: string | undefined;
            readonly error_message?: string | undefined;
        } | undefined;
        readonly "logout.failed"?: {
            readonly error_code?: string | undefined;
            readonly error_message?: string | undefined;
        } | undefined;
        readonly "organization.updated"?: {
            readonly id?: string | undefined;
            readonly changes_requested?: {
                readonly description?: string | undefined;
                readonly name?: string | undefined;
                readonly title?: string | undefined;
                readonly settings?: {
                    readonly threads_ui_visibility?: string | undefined;
                    readonly usage_dashboard_visibility?: string | undefined;
                } | undefined;
            } | undefined;
        } | undefined;
        readonly "project.created"?: {
            readonly id?: string | undefined;
            readonly data?: {
                readonly name?: string | undefined;
                readonly title?: string | undefined;
            } | undefined;
        } | undefined;
        readonly "project.updated"?: {
            readonly id?: string | undefined;
            readonly changes_requested?: {
                readonly title?: string | undefined;
            } | undefined;
        } | undefined;
        readonly "project.archived"?: {
            readonly id?: string | undefined;
        } | undefined;
        readonly "service_account.created"?: {
            readonly id?: string | undefined;
            readonly data?: {
                readonly role?: string | undefined;
            } | undefined;
        } | undefined;
        readonly "service_account.updated"?: {
            readonly id?: string | undefined;
            readonly changes_requested?: {
                readonly role?: string | undefined;
            } | undefined;
        } | undefined;
        readonly "service_account.deleted"?: {
            readonly id?: string | undefined;
        } | undefined;
        readonly "rate_limit.updated"?: {
            readonly id?: string | undefined;
            readonly changes_requested?: {
                readonly max_requests_per_1_minute?: number | undefined;
                readonly max_tokens_per_1_minute?: number | undefined;
                readonly max_images_per_1_minute?: number | undefined;
                readonly max_audio_megabytes_per_1_minute?: number | undefined;
                readonly max_requests_per_1_day?: number | undefined;
                readonly batch_1_day_max_input_tokens?: number | undefined;
            } | undefined;
        } | undefined;
        readonly "rate_limit.deleted"?: {
            readonly id?: string | undefined;
        } | undefined;
        readonly "user.added"?: {
            readonly id?: string | undefined;
            readonly data?: {
                readonly role?: string | undefined;
            } | undefined;
        } | undefined;
        readonly "user.updated"?: {
            readonly id?: string | undefined;
            readonly changes_requested?: {
                readonly role?: string | undefined;
            } | undefined;
        } | undefined;
        readonly "user.deleted"?: {
            readonly id?: string | undefined;
        } | undefined;
        readonly effective_at: number;
        readonly actor: {
            readonly type?: "session" | "api_key" | undefined;
            readonly session?: {
                readonly user?: {
                    readonly id?: string | undefined;
                    readonly email?: string | undefined;
                } | undefined;
                readonly ip_address?: string | undefined;
            } | undefined;
            readonly api_key?: {
                readonly type?: "user" | "service_account" | undefined;
                readonly id?: string | undefined;
                readonly user?: {
                    readonly id?: string | undefined;
                    readonly email?: string | undefined;
                } | undefined;
                readonly service_account?: {
                    readonly id?: string | undefined;
                } | undefined;
            } | undefined;
        };
    }[];
}, {}, {}>;
export declare class ListAuditLogsResponse extends ListAuditLogsResponse_base {
}
declare const UsageCostsParamsBucketWidth_base: S.Literal<["1d"]>;
export declare class UsageCostsParamsBucketWidth extends UsageCostsParamsBucketWidth_base {
}
declare const UsageCostsParams_base: S.Struct<{
    start_time: typeof S.Int;
    end_time: S.optionalWith<typeof S.Int, {
        nullable: true;
    }>;
    bucket_width: S.optionalWith<typeof UsageCostsParamsBucketWidth, {
        nullable: true;
        default: () => "1d";
    }>;
    project_ids: S.optionalWith<S.Array$<typeof S.String>, {
        nullable: true;
    }>;
    group_by: S.optionalWith<S.Array$<S.Literal<["project_id", "line_item"]>>, {
        nullable: true;
    }>;
    limit: S.optionalWith<typeof S.Int, {
        nullable: true;
        default: () => 7;
    }>;
    page: S.optionalWith<typeof S.String, {
        nullable: true;
    }>;
}>;
export declare class UsageCostsParams extends UsageCostsParams_base {
}
declare const UsageResponseObject_base: S.Literal<["page"]>;
export declare class UsageResponseObject extends UsageResponseObject_base {
}
declare const UsageTimeBucketObject_base: S.Literal<["bucket"]>;
export declare class UsageTimeBucketObject extends UsageTimeBucketObject_base {
}
declare const UsageCompletionsResultObject_base: S.Literal<["organization.usage.completions.result"]>;
export declare class UsageCompletionsResultObject extends UsageCompletionsResultObject_base {
}
declare const UsageCompletionsResult_base: S.Struct<{
    object: typeof UsageCompletionsResultObject;
    input_tokens: typeof S.Int;
    input_cached_tokens: S.optionalWith<typeof S.Int, {
        nullable: true;
    }>;
    output_tokens: typeof S.Int;
    input_audio_tokens: S.optionalWith<typeof S.Int, {
        nullable: true;
    }>;
    output_audio_tokens: S.optionalWith<typeof S.Int, {
        nullable: true;
    }>;
    num_model_requests: typeof S.Int;
    project_id: S.optionalWith<typeof S.String, {
        nullable: true;
    }>;
    user_id: S.optionalWith<typeof S.String, {
        nullable: true;
    }>;
    api_key_id: S.optionalWith<typeof S.String, {
        nullable: true;
    }>;
    model: S.optionalWith<typeof S.String, {
        nullable: true;
    }>;
    batch: S.optionalWith<typeof S.Boolean, {
        nullable: true;
    }>;
}>;
export declare class UsageCompletionsResult extends UsageCompletionsResult_base {
}
declare const UsageEmbeddingsResultObject_base: S.Literal<["organization.usage.embeddings.result"]>;
export declare class UsageEmbeddingsResultObject extends UsageEmbeddingsResultObject_base {
}
declare const UsageEmbeddingsResult_base: S.Struct<{
    object: typeof UsageEmbeddingsResultObject;
    input_tokens: typeof S.Int;
    num_model_requests: typeof S.Int;
    project_id: S.optionalWith<typeof S.String, {
        nullable: true;
    }>;
    user_id: S.optionalWith<typeof S.String, {
        nullable: true;
    }>;
    api_key_id: S.optionalWith<typeof S.String, {
        nullable: true;
    }>;
    model: S.optionalWith<typeof S.String, {
        nullable: true;
    }>;
}>;
export declare class UsageEmbeddingsResult extends UsageEmbeddingsResult_base {
}
declare const UsageModerationsResultObject_base: S.Literal<["organization.usage.moderations.result"]>;
export declare class UsageModerationsResultObject extends UsageModerationsResultObject_base {
}
declare const UsageModerationsResult_base: S.Struct<{
    object: typeof UsageModerationsResultObject;
    input_tokens: typeof S.Int;
    num_model_requests: typeof S.Int;
    project_id: S.optionalWith<typeof S.String, {
        nullable: true;
    }>;
    user_id: S.optionalWith<typeof S.String, {
        nullable: true;
    }>;
    api_key_id: S.optionalWith<typeof S.String, {
        nullable: true;
    }>;
    model: S.optionalWith<typeof S.String, {
        nullable: true;
    }>;
}>;
export declare class UsageModerationsResult extends UsageModerationsResult_base {
}
declare const UsageImagesResultObject_base: S.Literal<["organization.usage.images.result"]>;
export declare class UsageImagesResultObject extends UsageImagesResultObject_base {
}
declare const UsageImagesResult_base: S.Struct<{
    object: typeof UsageImagesResultObject;
    images: typeof S.Int;
    num_model_requests: typeof S.Int;
    source: S.optionalWith<typeof S.String, {
        nullable: true;
    }>;
    size: S.optionalWith<typeof S.String, {
        nullable: true;
    }>;
    project_id: S.optionalWith<typeof S.String, {
        nullable: true;
    }>;
    user_id: S.optionalWith<typeof S.String, {
        nullable: true;
    }>;
    api_key_id: S.optionalWith<typeof S.String, {
        nullable: true;
    }>;
    model: S.optionalWith<typeof S.String, {
        nullable: true;
    }>;
}>;
export declare class UsageImagesResult extends UsageImagesResult_base {
}
declare const UsageAudioSpeechesResultObject_base: S.Literal<["organization.usage.audio_speeches.result"]>;
export declare class UsageAudioSpeechesResultObject extends UsageAudioSpeechesResultObject_base {
}
declare const UsageAudioSpeechesResult_base: S.Struct<{
    object: typeof UsageAudioSpeechesResultObject;
    characters: typeof S.Int;
    num_model_requests: typeof S.Int;
    project_id: S.optionalWith<typeof S.String, {
        nullable: true;
    }>;
    user_id: S.optionalWith<typeof S.String, {
        nullable: true;
    }>;
    api_key_id: S.optionalWith<typeof S.String, {
        nullable: true;
    }>;
    model: S.optionalWith<typeof S.String, {
        nullable: true;
    }>;
}>;
export declare class UsageAudioSpeechesResult extends UsageAudioSpeechesResult_base {
}
declare const UsageAudioTranscriptionsResultObject_base: S.Literal<["organization.usage.audio_transcriptions.result"]>;
export declare class UsageAudioTranscriptionsResultObject extends UsageAudioTranscriptionsResultObject_base {
}
declare const UsageAudioTranscriptionsResult_base: S.Struct<{
    object: typeof UsageAudioTranscriptionsResultObject;
    seconds: typeof S.Int;
    num_model_requests: typeof S.Int;
    project_id: S.optionalWith<typeof S.String, {
        nullable: true;
    }>;
    user_id: S.optionalWith<typeof S.String, {
        nullable: true;
    }>;
    api_key_id: S.optionalWith<typeof S.String, {
        nullable: true;
    }>;
    model: S.optionalWith<typeof S.String, {
        nullable: true;
    }>;
}>;
export declare class UsageAudioTranscriptionsResult extends UsageAudioTranscriptionsResult_base {
}
declare const UsageVectorStoresResultObject_base: S.Literal<["organization.usage.vector_stores.result"]>;
export declare class UsageVectorStoresResultObject extends UsageVectorStoresResultObject_base {
}
declare const UsageVectorStoresResult_base: S.Struct<{
    object: typeof UsageVectorStoresResultObject;
    usage_bytes: typeof S.Int;
    project_id: S.optionalWith<typeof S.String, {
        nullable: true;
    }>;
}>;
export declare class UsageVectorStoresResult extends UsageVectorStoresResult_base {
}
declare const UsageCodeInterpreterSessionsResultObject_base: S.Literal<["organization.usage.code_interpreter_sessions.result"]>;
export declare class UsageCodeInterpreterSessionsResultObject extends UsageCodeInterpreterSessionsResultObject_base {
}
declare const UsageCodeInterpreterSessionsResult_base: S.Struct<{
    object: typeof UsageCodeInterpreterSessionsResultObject;
    num_sessions: S.optionalWith<typeof S.Int, {
        nullable: true;
    }>;
    project_id: S.optionalWith<typeof S.String, {
        nullable: true;
    }>;
}>;
export declare class UsageCodeInterpreterSessionsResult extends UsageCodeInterpreterSessionsResult_base {
}
declare const CostsResultObject_base: S.Literal<["organization.costs.result"]>;
export declare class CostsResultObject extends CostsResultObject_base {
}
declare const CostsResult_base: S.Struct<{
    object: typeof CostsResultObject;
    amount: S.optionalWith<S.Struct<{
        value: S.optionalWith<typeof S.Number, {
            nullable: true;
        }>;
        currency: S.optionalWith<typeof S.String, {
            nullable: true;
        }>;
    }>, {
        nullable: true;
    }>;
    line_item: S.optionalWith<typeof S.String, {
        nullable: true;
    }>;
    project_id: S.optionalWith<typeof S.String, {
        nullable: true;
    }>;
}>;
export declare class CostsResult extends CostsResult_base {
}
declare const UsageTimeBucket_base: S.Struct<{
    object: typeof UsageTimeBucketObject;
    start_time: typeof S.Int;
    end_time: typeof S.Int;
    result: S.Array$<S.Union<[typeof UsageCompletionsResult, typeof UsageEmbeddingsResult, typeof UsageModerationsResult, typeof UsageImagesResult, typeof UsageAudioSpeechesResult, typeof UsageAudioTranscriptionsResult, typeof UsageVectorStoresResult, typeof UsageCodeInterpreterSessionsResult, typeof CostsResult]>>;
}>;
export declare class UsageTimeBucket extends UsageTimeBucket_base {
}
declare const UsageResponse_base: S.Class<UsageResponse, {
    object: typeof UsageResponseObject;
    data: S.Array$<typeof UsageTimeBucket>;
    has_more: typeof S.Boolean;
    next_page: typeof S.String;
}, S.Struct.Encoded<{
    object: typeof UsageResponseObject;
    data: S.Array$<typeof UsageTimeBucket>;
    has_more: typeof S.Boolean;
    next_page: typeof S.String;
}>, never, {
    readonly object: "page";
} & {
    readonly has_more: boolean;
} & {
    readonly data: readonly {
        readonly object: "bucket";
        readonly start_time: number;
        readonly end_time: number;
        readonly result: readonly ({
            readonly object: "organization.usage.completions.result";
            readonly model?: string | undefined;
            readonly batch?: boolean | undefined;
            readonly project_id?: string | undefined;
            readonly input_tokens: number;
            readonly input_cached_tokens?: number | undefined;
            readonly output_tokens: number;
            readonly input_audio_tokens?: number | undefined;
            readonly output_audio_tokens?: number | undefined;
            readonly num_model_requests: number;
            readonly user_id?: string | undefined;
            readonly api_key_id?: string | undefined;
        } | {
            readonly object: "organization.usage.embeddings.result";
            readonly model?: string | undefined;
            readonly project_id?: string | undefined;
            readonly input_tokens: number;
            readonly num_model_requests: number;
            readonly user_id?: string | undefined;
            readonly api_key_id?: string | undefined;
        } | {
            readonly object: "organization.usage.moderations.result";
            readonly model?: string | undefined;
            readonly project_id?: string | undefined;
            readonly input_tokens: number;
            readonly num_model_requests: number;
            readonly user_id?: string | undefined;
            readonly api_key_id?: string | undefined;
        } | {
            readonly object: "organization.usage.images.result";
            readonly model?: string | undefined;
            readonly project_id?: string | undefined;
            readonly size?: string | undefined;
            readonly num_model_requests: number;
            readonly user_id?: string | undefined;
            readonly api_key_id?: string | undefined;
            readonly images: number;
            readonly source?: string | undefined;
        } | {
            readonly object: "organization.usage.audio_speeches.result";
            readonly model?: string | undefined;
            readonly project_id?: string | undefined;
            readonly num_model_requests: number;
            readonly user_id?: string | undefined;
            readonly api_key_id?: string | undefined;
            readonly characters: number;
        } | {
            readonly object: "organization.usage.audio_transcriptions.result";
            readonly model?: string | undefined;
            readonly project_id?: string | undefined;
            readonly num_model_requests: number;
            readonly user_id?: string | undefined;
            readonly api_key_id?: string | undefined;
            readonly seconds: number;
        } | {
            readonly object: "organization.usage.vector_stores.result";
            readonly project_id?: string | undefined;
            readonly usage_bytes: number;
        } | {
            readonly object: "organization.usage.code_interpreter_sessions.result";
            readonly project_id?: string | undefined;
            readonly num_sessions?: number | undefined;
        } | {
            readonly object: "organization.costs.result";
            readonly project_id?: string | undefined;
            readonly line_item?: string | undefined;
            readonly amount?: {
                readonly value?: number | undefined;
                readonly currency?: string | undefined;
            } | undefined;
        })[];
    }[];
} & {
    readonly next_page: string;
}, {}, {}>;
export declare class UsageResponse extends UsageResponse_base {
}
declare const ListInvitesParams_base: S.Struct<{
    limit: S.optionalWith<typeof S.Int, {
        nullable: true;
        default: () => 20;
    }>;
    after: S.optionalWith<typeof S.String, {
        nullable: true;
    }>;
}>;
export declare class ListInvitesParams extends ListInvitesParams_base {
}
declare const InviteListResponseObject_base: S.Literal<["list"]>;
export declare class InviteListResponseObject extends InviteListResponseObject_base {
}
declare const InviteObject_base: S.Literal<["organization.invite"]>;
export declare class InviteObject extends InviteObject_base {
}
declare const InviteRole_base: S.Literal<["owner", "reader"]>;
export declare class InviteRole extends InviteRole_base {
}
declare const InviteStatus_base: S.Literal<["accepted", "expired", "pending"]>;
export declare class InviteStatus extends InviteStatus_base {
}
declare const Invite_base: S.Struct<{
    object: typeof InviteObject;
    id: typeof S.String;
    email: typeof S.String;
    role: typeof InviteRole;
    status: typeof InviteStatus;
    invited_at: typeof S.Int;
    expires_at: typeof S.Int;
    accepted_at: S.optionalWith<typeof S.Int, {
        nullable: true;
    }>;
    projects: S.optionalWith<S.Array$<S.Struct<{
        id: S.optionalWith<typeof S.String, {
            nullable: true;
        }>;
        role: S.optionalWith<S.Literal<["member", "owner"]>, {
            nullable: true;
        }>;
    }>>, {
        nullable: true;
    }>;
}>;
export declare class Invite extends Invite_base {
}
declare const InviteListResponse_base: S.Class<InviteListResponse, {
    object: typeof InviteListResponseObject;
    data: S.Array$<typeof Invite>;
    first_id: S.optionalWith<typeof S.String, {
        nullable: true;
    }>;
    last_id: S.optionalWith<typeof S.String, {
        nullable: true;
    }>;
    has_more: S.optionalWith<typeof S.Boolean, {
        nullable: true;
    }>;
}, S.Struct.Encoded<{
    object: typeof InviteListResponseObject;
    data: S.Array$<typeof Invite>;
    first_id: S.optionalWith<typeof S.String, {
        nullable: true;
    }>;
    last_id: S.optionalWith<typeof S.String, {
        nullable: true;
    }>;
    has_more: S.optionalWith<typeof S.Boolean, {
        nullable: true;
    }>;
}>, never, {
    readonly object: "list";
} & {
    readonly first_id?: string | undefined;
} & {
    readonly last_id?: string | undefined;
} & {
    readonly has_more?: boolean | undefined;
} & {
    readonly data: readonly {
        readonly object: "organization.invite";
        readonly role: "owner" | "reader";
        readonly id: string;
        readonly status: "expired" | "accepted" | "pending";
        readonly expires_at: number;
        readonly email: string;
        readonly invited_at: number;
        readonly accepted_at?: number | undefined;
        readonly projects?: readonly {
            readonly role?: "owner" | "member" | undefined;
            readonly id?: string | undefined;
        }[] | undefined;
    }[];
}, {}, {}>;
export declare class InviteListResponse extends InviteListResponse_base {
}
declare const InviteRequestRole_base: S.Literal<["reader", "owner"]>;
export declare class InviteRequestRole extends InviteRequestRole_base {
}
declare const InviteRequest_base: S.Class<InviteRequest, {
    email: typeof S.String;
    role: typeof InviteRequestRole;
    projects: S.optionalWith<S.Array$<S.Struct<{
        id: typeof S.String;
        role: S.Literal<["member", "owner"]>;
    }>>, {
        nullable: true;
    }>;
}, S.Struct.Encoded<{
    email: typeof S.String;
    role: typeof InviteRequestRole;
    projects: S.optionalWith<S.Array$<S.Struct<{
        id: typeof S.String;
        role: S.Literal<["member", "owner"]>;
    }>>, {
        nullable: true;
    }>;
}>, never, {
    readonly role: "owner" | "reader";
} & {
    readonly email: string;
} & {
    readonly projects?: readonly {
        readonly role: "owner" | "member";
        readonly id: string;
    }[] | undefined;
}, {}, {}>;
export declare class InviteRequest extends InviteRequest_base {
}
declare const InviteDeleteResponseObject_base: S.Literal<["organization.invite.deleted"]>;
export declare class InviteDeleteResponseObject extends InviteDeleteResponseObject_base {
}
declare const InviteDeleteResponse_base: S.Class<InviteDeleteResponse, {
    object: typeof InviteDeleteResponseObject;
    id: typeof S.String;
    deleted: typeof S.Boolean;
}, S.Struct.Encoded<{
    object: typeof InviteDeleteResponseObject;
    id: typeof S.String;
    deleted: typeof S.Boolean;
}>, never, {
    readonly object: "organization.invite.deleted";
} & {
    readonly id: string;
} & {
    readonly deleted: boolean;
}, {}, {}>;
export declare class InviteDeleteResponse extends InviteDeleteResponse_base {
}
declare const ListProjectsParams_base: S.Struct<{
    limit: S.optionalWith<typeof S.Int, {
        nullable: true;
        default: () => 20;
    }>;
    after: S.optionalWith<typeof S.String, {
        nullable: true;
    }>;
    include_archived: S.optionalWith<typeof S.Boolean, {
        nullable: true;
        default: () => false;
    }>;
}>;
export declare class ListProjectsParams extends ListProjectsParams_base {
}
declare const ProjectListResponseObject_base: S.Literal<["list"]>;
export declare class ProjectListResponseObject extends ProjectListResponseObject_base {
}
declare const ProjectObject_base: S.Literal<["organization.project"]>;
export declare class ProjectObject extends ProjectObject_base {
}
declare const ProjectStatus_base: S.Literal<["active", "archived"]>;
export declare class ProjectStatus extends ProjectStatus_base {
}
declare const Project_base: S.Struct<{
    id: typeof S.String;
    object: typeof ProjectObject;
    name: typeof S.String;
    created_at: typeof S.Int;
    archived_at: S.optionalWith<typeof S.Int, {
        nullable: true;
    }>;
    status: typeof ProjectStatus;
}>;
export declare class Project extends Project_base {
}
declare const ProjectListResponse_base: S.Class<ProjectListResponse, {
    object: typeof ProjectListResponseObject;
    data: S.Array$<typeof Project>;
    first_id: typeof S.String;
    last_id: typeof S.String;
    has_more: typeof S.Boolean;
}, S.Struct.Encoded<{
    object: typeof ProjectListResponseObject;
    data: S.Array$<typeof Project>;
    first_id: typeof S.String;
    last_id: typeof S.String;
    has_more: typeof S.Boolean;
}>, never, {
    readonly object: "list";
} & {
    readonly first_id: string;
} & {
    readonly last_id: string;
} & {
    readonly has_more: boolean;
} & {
    readonly data: readonly {
        readonly object: "organization.project";
        readonly name: string;
        readonly id: string;
        readonly created_at: number;
        readonly status: "active" | "archived";
        readonly archived_at?: number | undefined;
    }[];
}, {}, {}>;
export declare class ProjectListResponse extends ProjectListResponse_base {
}
declare const ProjectCreateRequest_base: S.Class<ProjectCreateRequest, {
    name: typeof S.String;
}, S.Struct.Encoded<{
    name: typeof S.String;
}>, never, {
    readonly name: string;
}, {}, {}>;
export declare class ProjectCreateRequest extends ProjectCreateRequest_base {
}
declare const ProjectUpdateRequest_base: S.Class<ProjectUpdateRequest, {
    name: typeof S.String;
}, S.Struct.Encoded<{
    name: typeof S.String;
}>, never, {
    readonly name: string;
}, {}, {}>;
export declare class ProjectUpdateRequest extends ProjectUpdateRequest_base {
}
declare const Error_base: S.Struct<{
    code: S.NullOr<typeof S.String>;
    message: typeof S.String;
    param: S.NullOr<typeof S.String>;
    type: typeof S.String;
}>;
export declare class Error extends Error_base {
}
declare const ErrorResponse_base: S.Class<ErrorResponse, {
    error: typeof Error;
}, S.Struct.Encoded<{
    error: typeof Error;
}>, never, {
    readonly error: {
        readonly message: string;
        readonly type: string;
        readonly code: string | null;
        readonly param: string | null;
    };
}, {}, {}>;
export declare class ErrorResponse extends ErrorResponse_base {
}
declare const ListProjectApiKeysParams_base: S.Struct<{
    limit: S.optionalWith<typeof S.Int, {
        nullable: true;
        default: () => 20;
    }>;
    after: S.optionalWith<typeof S.String, {
        nullable: true;
    }>;
}>;
export declare class ListProjectApiKeysParams extends ListProjectApiKeysParams_base {
}
declare const ProjectApiKeyListResponseObject_base: S.Literal<["list"]>;
export declare class ProjectApiKeyListResponseObject extends ProjectApiKeyListResponseObject_base {
}
declare const ProjectApiKeyObject_base: S.Literal<["organization.project.api_key"]>;
export declare class ProjectApiKeyObject extends ProjectApiKeyObject_base {
}
declare const ProjectApiKeyOwnerType_base: S.Literal<["user", "service_account"]>;
export declare class ProjectApiKeyOwnerType extends ProjectApiKeyOwnerType_base {
}
declare const ProjectUserObject_base: S.Literal<["organization.project.user"]>;
export declare class ProjectUserObject extends ProjectUserObject_base {
}
declare const ProjectUserRole_base: S.Literal<["owner", "member"]>;
export declare class ProjectUserRole extends ProjectUserRole_base {
}
declare const ProjectUser_base: S.Struct<{
    object: typeof ProjectUserObject;
    id: typeof S.String;
    name: typeof S.String;
    email: typeof S.String;
    role: typeof ProjectUserRole;
    added_at: typeof S.Int;
}>;
export declare class ProjectUser extends ProjectUser_base {
}
declare const ProjectServiceAccountObject_base: S.Literal<["organization.project.service_account"]>;
export declare class ProjectServiceAccountObject extends ProjectServiceAccountObject_base {
}
declare const ProjectServiceAccountRole_base: S.Literal<["owner", "member"]>;
export declare class ProjectServiceAccountRole extends ProjectServiceAccountRole_base {
}
declare const ProjectServiceAccount_base: S.Struct<{
    object: typeof ProjectServiceAccountObject;
    id: typeof S.String;
    name: typeof S.String;
    role: typeof ProjectServiceAccountRole;
    created_at: typeof S.Int;
}>;
export declare class ProjectServiceAccount extends ProjectServiceAccount_base {
}
declare const ProjectApiKey_base: S.Struct<{
    object: typeof ProjectApiKeyObject;
    redacted_value: typeof S.String;
    name: typeof S.String;
    created_at: typeof S.Int;
    id: typeof S.String;
    owner: S.Struct<{
        type: S.optionalWith<typeof ProjectApiKeyOwnerType, {
            nullable: true;
        }>;
        user: S.optionalWith<typeof ProjectUser, {
            nullable: true;
        }>;
        service_account: S.optionalWith<typeof ProjectServiceAccount, {
            nullable: true;
        }>;
    }>;
}>;
export declare class ProjectApiKey extends ProjectApiKey_base {
}
declare const ProjectApiKeyListResponse_base: S.Class<ProjectApiKeyListResponse, {
    object: typeof ProjectApiKeyListResponseObject;
    data: S.Array$<typeof ProjectApiKey>;
    first_id: typeof S.String;
    last_id: typeof S.String;
    has_more: typeof S.Boolean;
}, S.Struct.Encoded<{
    object: typeof ProjectApiKeyListResponseObject;
    data: S.Array$<typeof ProjectApiKey>;
    first_id: typeof S.String;
    last_id: typeof S.String;
    has_more: typeof S.Boolean;
}>, never, {
    readonly object: "list";
} & {
    readonly first_id: string;
} & {
    readonly last_id: string;
} & {
    readonly has_more: boolean;
} & {
    readonly data: readonly {
        readonly object: "organization.project.api_key";
        readonly name: string;
        readonly id: string;
        readonly created_at: number;
        readonly redacted_value: string;
        readonly owner: {
            readonly type?: "user" | "service_account" | undefined;
            readonly user?: {
                readonly object: "organization.project.user";
                readonly role: "owner" | "member";
                readonly name: string;
                readonly id: string;
                readonly email: string;
                readonly added_at: number;
            } | undefined;
            readonly service_account?: {
                readonly object: "organization.project.service_account";
                readonly role: "owner" | "member";
                readonly name: string;
                readonly id: string;
                readonly created_at: number;
            } | undefined;
        };
    }[];
}, {}, {}>;
export declare class ProjectApiKeyListResponse extends ProjectApiKeyListResponse_base {
}
declare const ProjectApiKeyDeleteResponseObject_base: S.Literal<["organization.project.api_key.deleted"]>;
export declare class ProjectApiKeyDeleteResponseObject extends ProjectApiKeyDeleteResponseObject_base {
}
declare const ProjectApiKeyDeleteResponse_base: S.Class<ProjectApiKeyDeleteResponse, {
    object: typeof ProjectApiKeyDeleteResponseObject;
    id: typeof S.String;
    deleted: typeof S.Boolean;
}, S.Struct.Encoded<{
    object: typeof ProjectApiKeyDeleteResponseObject;
    id: typeof S.String;
    deleted: typeof S.Boolean;
}>, never, {
    readonly object: "organization.project.api_key.deleted";
} & {
    readonly id: string;
} & {
    readonly deleted: boolean;
}, {}, {}>;
export declare class ProjectApiKeyDeleteResponse extends ProjectApiKeyDeleteResponse_base {
}
declare const ListProjectRateLimitsParams_base: S.Struct<{
    limit: S.optionalWith<typeof S.Int, {
        nullable: true;
        default: () => 100;
    }>;
    after: S.optionalWith<typeof S.String, {
        nullable: true;
    }>;
    before: S.optionalWith<typeof S.String, {
        nullable: true;
    }>;
}>;
export declare class ListProjectRateLimitsParams extends ListProjectRateLimitsParams_base {
}
declare const ProjectRateLimitListResponseObject_base: S.Literal<["list"]>;
export declare class ProjectRateLimitListResponseObject extends ProjectRateLimitListResponseObject_base {
}
declare const ProjectRateLimitObject_base: S.Literal<["project.rate_limit"]>;
export declare class ProjectRateLimitObject extends ProjectRateLimitObject_base {
}
declare const ProjectRateLimit_base: S.Struct<{
    object: typeof ProjectRateLimitObject;
    id: typeof S.String;
    model: typeof S.String;
    max_requests_per_1_minute: typeof S.Int;
    max_tokens_per_1_minute: typeof S.Int;
    max_images_per_1_minute: S.optionalWith<typeof S.Int, {
        nullable: true;
    }>;
    max_audio_megabytes_per_1_minute: S.optionalWith<typeof S.Int, {
        nullable: true;
    }>;
    max_requests_per_1_day: S.optionalWith<typeof S.Int, {
        nullable: true;
    }>;
    batch_1_day_max_input_tokens: S.optionalWith<typeof S.Int, {
        nullable: true;
    }>;
}>;
export declare class ProjectRateLimit extends ProjectRateLimit_base {
}
declare const ProjectRateLimitListResponse_base: S.Class<ProjectRateLimitListResponse, {
    object: typeof ProjectRateLimitListResponseObject;
    data: S.Array$<typeof ProjectRateLimit>;
    first_id: typeof S.String;
    last_id: typeof S.String;
    has_more: typeof S.Boolean;
}, S.Struct.Encoded<{
    object: typeof ProjectRateLimitListResponseObject;
    data: S.Array$<typeof ProjectRateLimit>;
    first_id: typeof S.String;
    last_id: typeof S.String;
    has_more: typeof S.Boolean;
}>, never, {
    readonly object: "list";
} & {
    readonly first_id: string;
} & {
    readonly last_id: string;
} & {
    readonly has_more: boolean;
} & {
    readonly data: readonly {
        readonly object: "project.rate_limit";
        readonly model: string;
        readonly id: string;
        readonly max_requests_per_1_minute: number;
        readonly max_tokens_per_1_minute: number;
        readonly max_images_per_1_minute?: number | undefined;
        readonly max_audio_megabytes_per_1_minute?: number | undefined;
        readonly max_requests_per_1_day?: number | undefined;
        readonly batch_1_day_max_input_tokens?: number | undefined;
    }[];
}, {}, {}>;
export declare class ProjectRateLimitListResponse extends ProjectRateLimitListResponse_base {
}
declare const ProjectRateLimitUpdateRequest_base: S.Class<ProjectRateLimitUpdateRequest, {
    max_requests_per_1_minute: S.optionalWith<typeof S.Int, {
        nullable: true;
    }>;
    max_tokens_per_1_minute: S.optionalWith<typeof S.Int, {
        nullable: true;
    }>;
    max_images_per_1_minute: S.optionalWith<typeof S.Int, {
        nullable: true;
    }>;
    max_audio_megabytes_per_1_minute: S.optionalWith<typeof S.Int, {
        nullable: true;
    }>;
    max_requests_per_1_day: S.optionalWith<typeof S.Int, {
        nullable: true;
    }>;
    batch_1_day_max_input_tokens: S.optionalWith<typeof S.Int, {
        nullable: true;
    }>;
}, S.Struct.Encoded<{
    max_requests_per_1_minute: S.optionalWith<typeof S.Int, {
        nullable: true;
    }>;
    max_tokens_per_1_minute: S.optionalWith<typeof S.Int, {
        nullable: true;
    }>;
    max_images_per_1_minute: S.optionalWith<typeof S.Int, {
        nullable: true;
    }>;
    max_audio_megabytes_per_1_minute: S.optionalWith<typeof S.Int, {
        nullable: true;
    }>;
    max_requests_per_1_day: S.optionalWith<typeof S.Int, {
        nullable: true;
    }>;
    batch_1_day_max_input_tokens: S.optionalWith<typeof S.Int, {
        nullable: true;
    }>;
}>, never, {
    readonly max_requests_per_1_minute?: number | undefined;
} & {
    readonly max_tokens_per_1_minute?: number | undefined;
} & {
    readonly max_images_per_1_minute?: number | undefined;
} & {
    readonly max_audio_megabytes_per_1_minute?: number | undefined;
} & {
    readonly max_requests_per_1_day?: number | undefined;
} & {
    readonly batch_1_day_max_input_tokens?: number | undefined;
}, {}, {}>;
export declare class ProjectRateLimitUpdateRequest extends ProjectRateLimitUpdateRequest_base {
}
declare const ListProjectServiceAccountsParams_base: S.Struct<{
    limit: S.optionalWith<typeof S.Int, {
        nullable: true;
        default: () => 20;
    }>;
    after: S.optionalWith<typeof S.String, {
        nullable: true;
    }>;
}>;
export declare class ListProjectServiceAccountsParams extends ListProjectServiceAccountsParams_base {
}
declare const ProjectServiceAccountListResponseObject_base: S.Literal<["list"]>;
export declare class ProjectServiceAccountListResponseObject extends ProjectServiceAccountListResponseObject_base {
}
declare const ProjectServiceAccountListResponse_base: S.Class<ProjectServiceAccountListResponse, {
    object: typeof ProjectServiceAccountListResponseObject;
    data: S.Array$<typeof ProjectServiceAccount>;
    first_id: typeof S.String;
    last_id: typeof S.String;
    has_more: typeof S.Boolean;
}, S.Struct.Encoded<{
    object: typeof ProjectServiceAccountListResponseObject;
    data: S.Array$<typeof ProjectServiceAccount>;
    first_id: typeof S.String;
    last_id: typeof S.String;
    has_more: typeof S.Boolean;
}>, never, {
    readonly object: "list";
} & {
    readonly first_id: string;
} & {
    readonly last_id: string;
} & {
    readonly has_more: boolean;
} & {
    readonly data: readonly {
        readonly object: "organization.project.service_account";
        readonly role: "owner" | "member";
        readonly name: string;
        readonly id: string;
        readonly created_at: number;
    }[];
}, {}, {}>;
export declare class ProjectServiceAccountListResponse extends ProjectServiceAccountListResponse_base {
}
declare const ProjectServiceAccountCreateRequest_base: S.Class<ProjectServiceAccountCreateRequest, {
    name: typeof S.String;
}, S.Struct.Encoded<{
    name: typeof S.String;
}>, never, {
    readonly name: string;
}, {}, {}>;
export declare class ProjectServiceAccountCreateRequest extends ProjectServiceAccountCreateRequest_base {
}
declare const ProjectServiceAccountCreateResponseObject_base: S.Literal<["organization.project.service_account"]>;
export declare class ProjectServiceAccountCreateResponseObject extends ProjectServiceAccountCreateResponseObject_base {
}
declare const ProjectServiceAccountCreateResponseRole_base: S.Literal<["member"]>;
export declare class ProjectServiceAccountCreateResponseRole extends ProjectServiceAccountCreateResponseRole_base {
}
declare const ProjectServiceAccountApiKeyObject_base: S.Literal<["organization.project.service_account.api_key"]>;
export declare class ProjectServiceAccountApiKeyObject extends ProjectServiceAccountApiKeyObject_base {
}
declare const ProjectServiceAccountApiKey_base: S.Struct<{
    object: typeof ProjectServiceAccountApiKeyObject;
    value: typeof S.String;
    name: typeof S.String;
    created_at: typeof S.Int;
    id: typeof S.String;
}>;
export declare class ProjectServiceAccountApiKey extends ProjectServiceAccountApiKey_base {
}
declare const ProjectServiceAccountCreateResponse_base: S.Class<ProjectServiceAccountCreateResponse, {
    object: typeof ProjectServiceAccountCreateResponseObject;
    id: typeof S.String;
    name: typeof S.String;
    role: typeof ProjectServiceAccountCreateResponseRole;
    created_at: typeof S.Int;
    api_key: typeof ProjectServiceAccountApiKey;
}, S.Struct.Encoded<{
    object: typeof ProjectServiceAccountCreateResponseObject;
    id: typeof S.String;
    name: typeof S.String;
    role: typeof ProjectServiceAccountCreateResponseRole;
    created_at: typeof S.Int;
    api_key: typeof ProjectServiceAccountApiKey;
}>, never, {
    readonly object: "organization.project.service_account";
} & {
    readonly role: "member";
} & {
    readonly name: string;
} & {
    readonly id: string;
} & {
    readonly created_at: number;
} & {
    readonly api_key: {
        readonly object: "organization.project.service_account.api_key";
        readonly value: string;
        readonly name: string;
        readonly id: string;
        readonly created_at: number;
    };
}, {}, {}>;
export declare class ProjectServiceAccountCreateResponse extends ProjectServiceAccountCreateResponse_base {
}
declare const ProjectServiceAccountDeleteResponseObject_base: S.Literal<["organization.project.service_account.deleted"]>;
export declare class ProjectServiceAccountDeleteResponseObject extends ProjectServiceAccountDeleteResponseObject_base {
}
declare const ProjectServiceAccountDeleteResponse_base: S.Class<ProjectServiceAccountDeleteResponse, {
    object: typeof ProjectServiceAccountDeleteResponseObject;
    id: typeof S.String;
    deleted: typeof S.Boolean;
}, S.Struct.Encoded<{
    object: typeof ProjectServiceAccountDeleteResponseObject;
    id: typeof S.String;
    deleted: typeof S.Boolean;
}>, never, {
    readonly object: "organization.project.service_account.deleted";
} & {
    readonly id: string;
} & {
    readonly deleted: boolean;
}, {}, {}>;
export declare class ProjectServiceAccountDeleteResponse extends ProjectServiceAccountDeleteResponse_base {
}
declare const ListProjectUsersParams_base: S.Struct<{
    limit: S.optionalWith<typeof S.Int, {
        nullable: true;
        default: () => 20;
    }>;
    after: S.optionalWith<typeof S.String, {
        nullable: true;
    }>;
}>;
export declare class ListProjectUsersParams extends ListProjectUsersParams_base {
}
declare const ProjectUserListResponse_base: S.Class<ProjectUserListResponse, {
    object: typeof S.String;
    data: S.Array$<typeof ProjectUser>;
    first_id: typeof S.String;
    last_id: typeof S.String;
    has_more: typeof S.Boolean;
}, S.Struct.Encoded<{
    object: typeof S.String;
    data: S.Array$<typeof ProjectUser>;
    first_id: typeof S.String;
    last_id: typeof S.String;
    has_more: typeof S.Boolean;
}>, never, {
    readonly object: string;
} & {
    readonly first_id: string;
} & {
    readonly last_id: string;
} & {
    readonly has_more: boolean;
} & {
    readonly data: readonly {
        readonly object: "organization.project.user";
        readonly role: "owner" | "member";
        readonly name: string;
        readonly id: string;
        readonly email: string;
        readonly added_at: number;
    }[];
}, {}, {}>;
export declare class ProjectUserListResponse extends ProjectUserListResponse_base {
}
declare const ProjectUserCreateRequestRole_base: S.Literal<["owner", "member"]>;
export declare class ProjectUserCreateRequestRole extends ProjectUserCreateRequestRole_base {
}
declare const ProjectUserCreateRequest_base: S.Class<ProjectUserCreateRequest, {
    user_id: typeof S.String;
    role: typeof ProjectUserCreateRequestRole;
}, S.Struct.Encoded<{
    user_id: typeof S.String;
    role: typeof ProjectUserCreateRequestRole;
}>, never, {
    readonly role: "owner" | "member";
} & {
    readonly user_id: string;
}, {}, {}>;
export declare class ProjectUserCreateRequest extends ProjectUserCreateRequest_base {
}
declare const ProjectUserUpdateRequestRole_base: S.Literal<["owner", "member"]>;
export declare class ProjectUserUpdateRequestRole extends ProjectUserUpdateRequestRole_base {
}
declare const ProjectUserUpdateRequest_base: S.Class<ProjectUserUpdateRequest, {
    role: typeof ProjectUserUpdateRequestRole;
}, S.Struct.Encoded<{
    role: typeof ProjectUserUpdateRequestRole;
}>, never, {
    readonly role: "owner" | "member";
}, {}, {}>;
export declare class ProjectUserUpdateRequest extends ProjectUserUpdateRequest_base {
}
declare const ProjectUserDeleteResponseObject_base: S.Literal<["organization.project.user.deleted"]>;
export declare class ProjectUserDeleteResponseObject extends ProjectUserDeleteResponseObject_base {
}
declare const ProjectUserDeleteResponse_base: S.Class<ProjectUserDeleteResponse, {
    object: typeof ProjectUserDeleteResponseObject;
    id: typeof S.String;
    deleted: typeof S.Boolean;
}, S.Struct.Encoded<{
    object: typeof ProjectUserDeleteResponseObject;
    id: typeof S.String;
    deleted: typeof S.Boolean;
}>, never, {
    readonly object: "organization.project.user.deleted";
} & {
    readonly id: string;
} & {
    readonly deleted: boolean;
}, {}, {}>;
export declare class ProjectUserDeleteResponse extends ProjectUserDeleteResponse_base {
}
declare const UsageAudioSpeechesParamsBucketWidth_base: S.Literal<["1m", "1h", "1d"]>;
export declare class UsageAudioSpeechesParamsBucketWidth extends UsageAudioSpeechesParamsBucketWidth_base {
}
declare const UsageAudioSpeechesParams_base: S.Struct<{
    start_time: typeof S.Int;
    end_time: S.optionalWith<typeof S.Int, {
        nullable: true;
    }>;
    bucket_width: S.optionalWith<typeof UsageAudioSpeechesParamsBucketWidth, {
        nullable: true;
        default: () => "1d";
    }>;
    project_ids: S.optionalWith<S.Array$<typeof S.String>, {
        nullable: true;
    }>;
    user_ids: S.optionalWith<S.Array$<typeof S.String>, {
        nullable: true;
    }>;
    api_key_ids: S.optionalWith<S.Array$<typeof S.String>, {
        nullable: true;
    }>;
    models: S.optionalWith<S.Array$<typeof S.String>, {
        nullable: true;
    }>;
    group_by: S.optionalWith<S.Array$<S.Literal<["project_id", "user_id", "api_key_id", "model"]>>, {
        nullable: true;
    }>;
    limit: S.optionalWith<typeof S.Int, {
        nullable: true;
    }>;
    page: S.optionalWith<typeof S.String, {
        nullable: true;
    }>;
}>;
export declare class UsageAudioSpeechesParams extends UsageAudioSpeechesParams_base {
}
declare const UsageAudioTranscriptionsParamsBucketWidth_base: S.Literal<["1m", "1h", "1d"]>;
export declare class UsageAudioTranscriptionsParamsBucketWidth extends UsageAudioTranscriptionsParamsBucketWidth_base {
}
declare const UsageAudioTranscriptionsParams_base: S.Struct<{
    start_time: typeof S.Int;
    end_time: S.optionalWith<typeof S.Int, {
        nullable: true;
    }>;
    bucket_width: S.optionalWith<typeof UsageAudioTranscriptionsParamsBucketWidth, {
        nullable: true;
        default: () => "1d";
    }>;
    project_ids: S.optionalWith<S.Array$<typeof S.String>, {
        nullable: true;
    }>;
    user_ids: S.optionalWith<S.Array$<typeof S.String>, {
        nullable: true;
    }>;
    api_key_ids: S.optionalWith<S.Array$<typeof S.String>, {
        nullable: true;
    }>;
    models: S.optionalWith<S.Array$<typeof S.String>, {
        nullable: true;
    }>;
    group_by: S.optionalWith<S.Array$<S.Literal<["project_id", "user_id", "api_key_id", "model"]>>, {
        nullable: true;
    }>;
    limit: S.optionalWith<typeof S.Int, {
        nullable: true;
    }>;
    page: S.optionalWith<typeof S.String, {
        nullable: true;
    }>;
}>;
export declare class UsageAudioTranscriptionsParams extends UsageAudioTranscriptionsParams_base {
}
declare const UsageCodeInterpreterSessionsParamsBucketWidth_base: S.Literal<["1m", "1h", "1d"]>;
export declare class UsageCodeInterpreterSessionsParamsBucketWidth extends UsageCodeInterpreterSessionsParamsBucketWidth_base {
}
declare const UsageCodeInterpreterSessionsParams_base: S.Struct<{
    start_time: typeof S.Int;
    end_time: S.optionalWith<typeof S.Int, {
        nullable: true;
    }>;
    bucket_width: S.optionalWith<typeof UsageCodeInterpreterSessionsParamsBucketWidth, {
        nullable: true;
        default: () => "1d";
    }>;
    project_ids: S.optionalWith<S.Array$<typeof S.String>, {
        nullable: true;
    }>;
    group_by: S.optionalWith<S.Array$<S.Literal<["project_id"]>>, {
        nullable: true;
    }>;
    limit: S.optionalWith<typeof S.Int, {
        nullable: true;
    }>;
    page: S.optionalWith<typeof S.String, {
        nullable: true;
    }>;
}>;
export declare class UsageCodeInterpreterSessionsParams extends UsageCodeInterpreterSessionsParams_base {
}
declare const UsageCompletionsParamsBucketWidth_base: S.Literal<["1m", "1h", "1d"]>;
export declare class UsageCompletionsParamsBucketWidth extends UsageCompletionsParamsBucketWidth_base {
}
declare const UsageCompletionsParams_base: S.Struct<{
    start_time: typeof S.Int;
    end_time: S.optionalWith<typeof S.Int, {
        nullable: true;
    }>;
    bucket_width: S.optionalWith<typeof UsageCompletionsParamsBucketWidth, {
        nullable: true;
        default: () => "1d";
    }>;
    project_ids: S.optionalWith<S.Array$<typeof S.String>, {
        nullable: true;
    }>;
    user_ids: S.optionalWith<S.Array$<typeof S.String>, {
        nullable: true;
    }>;
    api_key_ids: S.optionalWith<S.Array$<typeof S.String>, {
        nullable: true;
    }>;
    models: S.optionalWith<S.Array$<typeof S.String>, {
        nullable: true;
    }>;
    batch: S.optionalWith<typeof S.Boolean, {
        nullable: true;
    }>;
    group_by: S.optionalWith<S.Array$<S.Literal<["project_id", "user_id", "api_key_id", "model", "batch"]>>, {
        nullable: true;
    }>;
    limit: S.optionalWith<typeof S.Int, {
        nullable: true;
    }>;
    page: S.optionalWith<typeof S.String, {
        nullable: true;
    }>;
}>;
export declare class UsageCompletionsParams extends UsageCompletionsParams_base {
}
declare const UsageEmbeddingsParamsBucketWidth_base: S.Literal<["1m", "1h", "1d"]>;
export declare class UsageEmbeddingsParamsBucketWidth extends UsageEmbeddingsParamsBucketWidth_base {
}
declare const UsageEmbeddingsParams_base: S.Struct<{
    start_time: typeof S.Int;
    end_time: S.optionalWith<typeof S.Int, {
        nullable: true;
    }>;
    bucket_width: S.optionalWith<typeof UsageEmbeddingsParamsBucketWidth, {
        nullable: true;
        default: () => "1d";
    }>;
    project_ids: S.optionalWith<S.Array$<typeof S.String>, {
        nullable: true;
    }>;
    user_ids: S.optionalWith<S.Array$<typeof S.String>, {
        nullable: true;
    }>;
    api_key_ids: S.optionalWith<S.Array$<typeof S.String>, {
        nullable: true;
    }>;
    models: S.optionalWith<S.Array$<typeof S.String>, {
        nullable: true;
    }>;
    group_by: S.optionalWith<S.Array$<S.Literal<["project_id", "user_id", "api_key_id", "model"]>>, {
        nullable: true;
    }>;
    limit: S.optionalWith<typeof S.Int, {
        nullable: true;
    }>;
    page: S.optionalWith<typeof S.String, {
        nullable: true;
    }>;
}>;
export declare class UsageEmbeddingsParams extends UsageEmbeddingsParams_base {
}
declare const UsageImagesParamsBucketWidth_base: S.Literal<["1m", "1h", "1d"]>;
export declare class UsageImagesParamsBucketWidth extends UsageImagesParamsBucketWidth_base {
}
declare const UsageImagesParams_base: S.Struct<{
    start_time: typeof S.Int;
    end_time: S.optionalWith<typeof S.Int, {
        nullable: true;
    }>;
    bucket_width: S.optionalWith<typeof UsageImagesParamsBucketWidth, {
        nullable: true;
        default: () => "1d";
    }>;
    sources: S.optionalWith<S.Array$<S.Literal<["image.generation", "image.edit", "image.variation"]>>, {
        nullable: true;
    }>;
    sizes: S.optionalWith<S.Array$<S.Literal<["256x256", "512x512", "1024x1024", "1792x1792", "1024x1792"]>>, {
        nullable: true;
    }>;
    project_ids: S.optionalWith<S.Array$<typeof S.String>, {
        nullable: true;
    }>;
    user_ids: S.optionalWith<S.Array$<typeof S.String>, {
        nullable: true;
    }>;
    api_key_ids: S.optionalWith<S.Array$<typeof S.String>, {
        nullable: true;
    }>;
    models: S.optionalWith<S.Array$<typeof S.String>, {
        nullable: true;
    }>;
    group_by: S.optionalWith<S.Array$<S.Literal<["project_id", "user_id", "api_key_id", "model", "size", "source"]>>, {
        nullable: true;
    }>;
    limit: S.optionalWith<typeof S.Int, {
        nullable: true;
    }>;
    page: S.optionalWith<typeof S.String, {
        nullable: true;
    }>;
}>;
export declare class UsageImagesParams extends UsageImagesParams_base {
}
declare const UsageModerationsParamsBucketWidth_base: S.Literal<["1m", "1h", "1d"]>;
export declare class UsageModerationsParamsBucketWidth extends UsageModerationsParamsBucketWidth_base {
}
declare const UsageModerationsParams_base: S.Struct<{
    start_time: typeof S.Int;
    end_time: S.optionalWith<typeof S.Int, {
        nullable: true;
    }>;
    bucket_width: S.optionalWith<typeof UsageModerationsParamsBucketWidth, {
        nullable: true;
        default: () => "1d";
    }>;
    project_ids: S.optionalWith<S.Array$<typeof S.String>, {
        nullable: true;
    }>;
    user_ids: S.optionalWith<S.Array$<typeof S.String>, {
        nullable: true;
    }>;
    api_key_ids: S.optionalWith<S.Array$<typeof S.String>, {
        nullable: true;
    }>;
    models: S.optionalWith<S.Array$<typeof S.String>, {
        nullable: true;
    }>;
    group_by: S.optionalWith<S.Array$<S.Literal<["project_id", "user_id", "api_key_id", "model"]>>, {
        nullable: true;
    }>;
    limit: S.optionalWith<typeof S.Int, {
        nullable: true;
    }>;
    page: S.optionalWith<typeof S.String, {
        nullable: true;
    }>;
}>;
export declare class UsageModerationsParams extends UsageModerationsParams_base {
}
declare const UsageVectorStoresParamsBucketWidth_base: S.Literal<["1m", "1h", "1d"]>;
export declare class UsageVectorStoresParamsBucketWidth extends UsageVectorStoresParamsBucketWidth_base {
}
declare const UsageVectorStoresParams_base: S.Struct<{
    start_time: typeof S.Int;
    end_time: S.optionalWith<typeof S.Int, {
        nullable: true;
    }>;
    bucket_width: S.optionalWith<typeof UsageVectorStoresParamsBucketWidth, {
        nullable: true;
        default: () => "1d";
    }>;
    project_ids: S.optionalWith<S.Array$<typeof S.String>, {
        nullable: true;
    }>;
    group_by: S.optionalWith<S.Array$<S.Literal<["project_id"]>>, {
        nullable: true;
    }>;
    limit: S.optionalWith<typeof S.Int, {
        nullable: true;
    }>;
    page: S.optionalWith<typeof S.String, {
        nullable: true;
    }>;
}>;
export declare class UsageVectorStoresParams extends UsageVectorStoresParams_base {
}
declare const ListUsersParams_base: S.Struct<{
    limit: S.optionalWith<typeof S.Int, {
        nullable: true;
        default: () => 20;
    }>;
    after: S.optionalWith<typeof S.String, {
        nullable: true;
    }>;
    emails: S.optionalWith<S.Array$<typeof S.String>, {
        nullable: true;
    }>;
}>;
export declare class ListUsersParams extends ListUsersParams_base {
}
declare const UserListResponseObject_base: S.Literal<["list"]>;
export declare class UserListResponseObject extends UserListResponseObject_base {
}
declare const UserObject_base: S.Literal<["organization.user"]>;
export declare class UserObject extends UserObject_base {
}
declare const UserRole_base: S.Literal<["owner", "reader"]>;
export declare class UserRole extends UserRole_base {
}
declare const User_base: S.Struct<{
    object: typeof UserObject;
    id: typeof S.String;
    name: typeof S.String;
    email: typeof S.String;
    role: typeof UserRole;
    added_at: typeof S.Int;
}>;
export declare class User extends User_base {
}
declare const UserListResponse_base: S.Class<UserListResponse, {
    object: typeof UserListResponseObject;
    data: S.Array$<typeof User>;
    first_id: typeof S.String;
    last_id: typeof S.String;
    has_more: typeof S.Boolean;
}, S.Struct.Encoded<{
    object: typeof UserListResponseObject;
    data: S.Array$<typeof User>;
    first_id: typeof S.String;
    last_id: typeof S.String;
    has_more: typeof S.Boolean;
}>, never, {
    readonly object: "list";
} & {
    readonly first_id: string;
} & {
    readonly last_id: string;
} & {
    readonly has_more: boolean;
} & {
    readonly data: readonly {
        readonly object: "organization.user";
        readonly role: "owner" | "reader";
        readonly name: string;
        readonly id: string;
        readonly email: string;
        readonly added_at: number;
    }[];
}, {}, {}>;
export declare class UserListResponse extends UserListResponse_base {
}
declare const UserRoleUpdateRequestRole_base: S.Literal<["owner", "reader"]>;
export declare class UserRoleUpdateRequestRole extends UserRoleUpdateRequestRole_base {
}
declare const UserRoleUpdateRequest_base: S.Class<UserRoleUpdateRequest, {
    role: typeof UserRoleUpdateRequestRole;
}, S.Struct.Encoded<{
    role: typeof UserRoleUpdateRequestRole;
}>, never, {
    readonly role: "owner" | "reader";
}, {}, {}>;
export declare class UserRoleUpdateRequest extends UserRoleUpdateRequest_base {
}
declare const UserDeleteResponseObject_base: S.Literal<["organization.user.deleted"]>;
export declare class UserDeleteResponseObject extends UserDeleteResponseObject_base {
}
declare const UserDeleteResponse_base: S.Class<UserDeleteResponse, {
    object: typeof UserDeleteResponseObject;
    id: typeof S.String;
    deleted: typeof S.Boolean;
}, S.Struct.Encoded<{
    object: typeof UserDeleteResponseObject;
    id: typeof S.String;
    deleted: typeof S.Boolean;
}>, never, {
    readonly object: "organization.user.deleted";
} & {
    readonly id: string;
} & {
    readonly deleted: boolean;
}, {}, {}>;
export declare class UserDeleteResponse extends UserDeleteResponse_base {
}
declare const RealtimeSessionCreateRequestModel_base: S.Literal<["gpt-4o-realtime-preview", "gpt-4o-realtime-preview-2024-10-01", "gpt-4o-realtime-preview-2024-12-17", "gpt-4o-mini-realtime-preview", "gpt-4o-mini-realtime-preview-2024-12-17"]>;
export declare class RealtimeSessionCreateRequestModel extends RealtimeSessionCreateRequestModel_base {
}
declare const RealtimeSessionCreateRequestInputAudioFormat_base: S.Literal<["pcm16", "g711_ulaw", "g711_alaw"]>;
export declare class RealtimeSessionCreateRequestInputAudioFormat extends RealtimeSessionCreateRequestInputAudioFormat_base {
}
declare const RealtimeSessionCreateRequestOutputAudioFormat_base: S.Literal<["pcm16", "g711_ulaw", "g711_alaw"]>;
export declare class RealtimeSessionCreateRequestOutputAudioFormat extends RealtimeSessionCreateRequestOutputAudioFormat_base {
}
declare const RealtimeSessionCreateRequestTurnDetectionType_base: S.Literal<["server_vad", "semantic_vad"]>;
export declare class RealtimeSessionCreateRequestTurnDetectionType extends RealtimeSessionCreateRequestTurnDetectionType_base {
}
declare const RealtimeSessionCreateRequestTurnDetectionEagerness_base: S.Literal<["low", "medium", "high", "auto"]>;
export declare class RealtimeSessionCreateRequestTurnDetectionEagerness extends RealtimeSessionCreateRequestTurnDetectionEagerness_base {
}
declare const RealtimeSessionCreateRequestInputAudioNoiseReductionType_base: S.Literal<["near_field", "far_field"]>;
export declare class RealtimeSessionCreateRequestInputAudioNoiseReductionType extends RealtimeSessionCreateRequestInputAudioNoiseReductionType_base {
}
declare const RealtimeSessionCreateRequestMaxResponseOutputTokensEnum_base: S.Literal<["inf"]>;
export declare class RealtimeSessionCreateRequestMaxResponseOutputTokensEnum extends RealtimeSessionCreateRequestMaxResponseOutputTokensEnum_base {
}
declare const RealtimeSessionCreateRequest_base: S.Class<RealtimeSessionCreateRequest, {
    model: S.optionalWith<typeof RealtimeSessionCreateRequestModel, {
        nullable: true;
    }>;
    instructions: S.optionalWith<typeof S.String, {
        nullable: true;
    }>;
    voice: S.optionalWith<typeof VoiceIdsShared, {
        nullable: true;
    }>;
    input_audio_format: S.optionalWith<typeof RealtimeSessionCreateRequestInputAudioFormat, {
        nullable: true;
        default: () => "pcm16";
    }>;
    output_audio_format: S.optionalWith<typeof RealtimeSessionCreateRequestOutputAudioFormat, {
        nullable: true;
        default: () => "pcm16";
    }>;
    input_audio_transcription: S.optionalWith<S.Struct<{
        model: S.optionalWith<typeof S.String, {
            nullable: true;
        }>;
        language: S.optionalWith<typeof S.String, {
            nullable: true;
        }>;
        prompt: S.optionalWith<typeof S.String, {
            nullable: true;
        }>;
    }>, {
        nullable: true;
    }>;
    turn_detection: S.optionalWith<S.Struct<{
        type: S.optionalWith<typeof RealtimeSessionCreateRequestTurnDetectionType, {
            nullable: true;
            default: () => "server_vad";
        }>;
        eagerness: S.optionalWith<typeof RealtimeSessionCreateRequestTurnDetectionEagerness, {
            nullable: true;
            default: () => "auto";
        }>;
        threshold: S.optionalWith<typeof S.Number, {
            nullable: true;
        }>;
        prefix_padding_ms: S.optionalWith<typeof S.Int, {
            nullable: true;
        }>;
        silence_duration_ms: S.optionalWith<typeof S.Int, {
            nullable: true;
        }>;
        create_response: S.optionalWith<typeof S.Boolean, {
            nullable: true;
            default: () => true;
        }>;
        interrupt_response: S.optionalWith<typeof S.Boolean, {
            nullable: true;
            default: () => true;
        }>;
    }>, {
        nullable: true;
    }>;
    input_audio_noise_reduction: S.optionalWith<S.NullOr<S.Struct<{
        type: S.optionalWith<typeof RealtimeSessionCreateRequestInputAudioNoiseReductionType, {
            nullable: true;
        }>;
    }>>, {
        default: () => null;
    }>;
    tools: S.optionalWith<S.Array$<S.Struct<{
        type: S.optionalWith<S.Literal<["function"]>, {
            nullable: true;
        }>;
        name: S.optionalWith<typeof S.String, {
            nullable: true;
        }>;
        description: S.optionalWith<typeof S.String, {
            nullable: true;
        }>;
        parameters: S.optionalWith<S.Record$<typeof S.String, typeof S.Unknown>, {
            nullable: true;
        }>;
    }>>, {
        nullable: true;
    }>;
    tool_choice: S.optionalWith<typeof S.String, {
        nullable: true;
        default: () => "auto";
    }>;
    temperature: S.optionalWith<typeof S.Number, {
        nullable: true;
        default: () => 0.8;
    }>;
    max_response_output_tokens: S.optionalWith<S.Union<[typeof S.Int, typeof RealtimeSessionCreateRequestMaxResponseOutputTokensEnum]>, {
        nullable: true;
    }>;
}, S.Struct.Encoded<{
    model: S.optionalWith<typeof RealtimeSessionCreateRequestModel, {
        nullable: true;
    }>;
    instructions: S.optionalWith<typeof S.String, {
        nullable: true;
    }>;
    voice: S.optionalWith<typeof VoiceIdsShared, {
        nullable: true;
    }>;
    input_audio_format: S.optionalWith<typeof RealtimeSessionCreateRequestInputAudioFormat, {
        nullable: true;
        default: () => "pcm16";
    }>;
    output_audio_format: S.optionalWith<typeof RealtimeSessionCreateRequestOutputAudioFormat, {
        nullable: true;
        default: () => "pcm16";
    }>;
    input_audio_transcription: S.optionalWith<S.Struct<{
        model: S.optionalWith<typeof S.String, {
            nullable: true;
        }>;
        language: S.optionalWith<typeof S.String, {
            nullable: true;
        }>;
        prompt: S.optionalWith<typeof S.String, {
            nullable: true;
        }>;
    }>, {
        nullable: true;
    }>;
    turn_detection: S.optionalWith<S.Struct<{
        type: S.optionalWith<typeof RealtimeSessionCreateRequestTurnDetectionType, {
            nullable: true;
            default: () => "server_vad";
        }>;
        eagerness: S.optionalWith<typeof RealtimeSessionCreateRequestTurnDetectionEagerness, {
            nullable: true;
            default: () => "auto";
        }>;
        threshold: S.optionalWith<typeof S.Number, {
            nullable: true;
        }>;
        prefix_padding_ms: S.optionalWith<typeof S.Int, {
            nullable: true;
        }>;
        silence_duration_ms: S.optionalWith<typeof S.Int, {
            nullable: true;
        }>;
        create_response: S.optionalWith<typeof S.Boolean, {
            nullable: true;
            default: () => true;
        }>;
        interrupt_response: S.optionalWith<typeof S.Boolean, {
            nullable: true;
            default: () => true;
        }>;
    }>, {
        nullable: true;
    }>;
    input_audio_noise_reduction: S.optionalWith<S.NullOr<S.Struct<{
        type: S.optionalWith<typeof RealtimeSessionCreateRequestInputAudioNoiseReductionType, {
            nullable: true;
        }>;
    }>>, {
        default: () => null;
    }>;
    tools: S.optionalWith<S.Array$<S.Struct<{
        type: S.optionalWith<S.Literal<["function"]>, {
            nullable: true;
        }>;
        name: S.optionalWith<typeof S.String, {
            nullable: true;
        }>;
        description: S.optionalWith<typeof S.String, {
            nullable: true;
        }>;
        parameters: S.optionalWith<S.Record$<typeof S.String, typeof S.Unknown>, {
            nullable: true;
        }>;
    }>>, {
        nullable: true;
    }>;
    tool_choice: S.optionalWith<typeof S.String, {
        nullable: true;
        default: () => "auto";
    }>;
    temperature: S.optionalWith<typeof S.Number, {
        nullable: true;
        default: () => 0.8;
    }>;
    max_response_output_tokens: S.optionalWith<S.Union<[typeof S.Int, typeof RealtimeSessionCreateRequestMaxResponseOutputTokensEnum]>, {
        nullable: true;
    }>;
}>, never, {
    readonly model?: "gpt-4o-realtime-preview" | "gpt-4o-realtime-preview-2024-10-01" | "gpt-4o-realtime-preview-2024-12-17" | "gpt-4o-mini-realtime-preview" | "gpt-4o-mini-realtime-preview-2024-12-17" | undefined;
} & {
    readonly instructions?: string | undefined;
} & {
    readonly tools?: readonly {
        readonly description?: string | undefined;
        readonly type?: "function" | undefined;
        readonly name?: string | undefined;
        readonly parameters?: {
            readonly [x: string]: unknown;
        } | undefined;
    }[] | undefined;
} & {
    readonly temperature?: number;
} & {
    readonly voice?: string | undefined;
} & {
    readonly tool_choice?: string;
} & {
    readonly input_audio_format?: "pcm16" | "g711_ulaw" | "g711_alaw";
} & {
    readonly output_audio_format?: "pcm16" | "g711_ulaw" | "g711_alaw";
} & {
    readonly input_audio_transcription?: {
        readonly model?: string | undefined;
        readonly language?: string | undefined;
        readonly prompt?: string | undefined;
    } | undefined;
} & {
    readonly turn_detection?: {
        readonly type: "server_vad" | "semantic_vad";
        readonly eagerness: "auto" | "low" | "medium" | "high";
        readonly threshold?: number | undefined;
        readonly prefix_padding_ms?: number | undefined;
        readonly silence_duration_ms?: number | undefined;
        readonly create_response: boolean;
        readonly interrupt_response: boolean;
    } | undefined;
} & {
    readonly input_audio_noise_reduction?: {
        readonly type?: "near_field" | "far_field" | undefined;
    } | null;
} & {
    readonly max_response_output_tokens?: number | "inf" | undefined;
}, {}, {}>;
export declare class RealtimeSessionCreateRequest extends RealtimeSessionCreateRequest_base {
}
declare const RealtimeSessionCreateResponseMaxResponseOutputTokensEnum_base: S.Literal<["inf"]>;
export declare class RealtimeSessionCreateResponseMaxResponseOutputTokensEnum extends RealtimeSessionCreateResponseMaxResponseOutputTokensEnum_base {
}
declare const RealtimeSessionCreateResponse_base: S.Class<RealtimeSessionCreateResponse, {
    client_secret: S.Struct<{
        value: typeof S.String;
        expires_at: typeof S.Int;
    }>;
    instructions: S.optionalWith<typeof S.String, {
        nullable: true;
    }>;
    voice: S.optionalWith<typeof VoiceIdsShared, {
        nullable: true;
    }>;
    input_audio_format: S.optionalWith<typeof S.String, {
        nullable: true;
    }>;
    output_audio_format: S.optionalWith<typeof S.String, {
        nullable: true;
    }>;
    input_audio_transcription: S.optionalWith<S.Struct<{
        model: S.optionalWith<typeof S.String, {
            nullable: true;
        }>;
    }>, {
        nullable: true;
    }>;
    turn_detection: S.optionalWith<S.Struct<{
        type: S.optionalWith<typeof S.String, {
            nullable: true;
        }>;
        threshold: S.optionalWith<typeof S.Number, {
            nullable: true;
        }>;
        prefix_padding_ms: S.optionalWith<typeof S.Int, {
            nullable: true;
        }>;
        silence_duration_ms: S.optionalWith<typeof S.Int, {
            nullable: true;
        }>;
    }>, {
        nullable: true;
    }>;
    tools: S.optionalWith<S.Array$<S.Struct<{
        type: S.optionalWith<S.Literal<["function"]>, {
            nullable: true;
        }>;
        name: S.optionalWith<typeof S.String, {
            nullable: true;
        }>;
        description: S.optionalWith<typeof S.String, {
            nullable: true;
        }>;
        parameters: S.optionalWith<S.Record$<typeof S.String, typeof S.Unknown>, {
            nullable: true;
        }>;
    }>>, {
        nullable: true;
    }>;
    tool_choice: S.optionalWith<typeof S.String, {
        nullable: true;
    }>;
    temperature: S.optionalWith<typeof S.Number, {
        nullable: true;
    }>;
    max_response_output_tokens: S.optionalWith<S.Union<[typeof S.Int, typeof RealtimeSessionCreateResponseMaxResponseOutputTokensEnum]>, {
        nullable: true;
    }>;
}, S.Struct.Encoded<{
    client_secret: S.Struct<{
        value: typeof S.String;
        expires_at: typeof S.Int;
    }>;
    instructions: S.optionalWith<typeof S.String, {
        nullable: true;
    }>;
    voice: S.optionalWith<typeof VoiceIdsShared, {
        nullable: true;
    }>;
    input_audio_format: S.optionalWith<typeof S.String, {
        nullable: true;
    }>;
    output_audio_format: S.optionalWith<typeof S.String, {
        nullable: true;
    }>;
    input_audio_transcription: S.optionalWith<S.Struct<{
        model: S.optionalWith<typeof S.String, {
            nullable: true;
        }>;
    }>, {
        nullable: true;
    }>;
    turn_detection: S.optionalWith<S.Struct<{
        type: S.optionalWith<typeof S.String, {
            nullable: true;
        }>;
        threshold: S.optionalWith<typeof S.Number, {
            nullable: true;
        }>;
        prefix_padding_ms: S.optionalWith<typeof S.Int, {
            nullable: true;
        }>;
        silence_duration_ms: S.optionalWith<typeof S.Int, {
            nullable: true;
        }>;
    }>, {
        nullable: true;
    }>;
    tools: S.optionalWith<S.Array$<S.Struct<{
        type: S.optionalWith<S.Literal<["function"]>, {
            nullable: true;
        }>;
        name: S.optionalWith<typeof S.String, {
            nullable: true;
        }>;
        description: S.optionalWith<typeof S.String, {
            nullable: true;
        }>;
        parameters: S.optionalWith<S.Record$<typeof S.String, typeof S.Unknown>, {
            nullable: true;
        }>;
    }>>, {
        nullable: true;
    }>;
    tool_choice: S.optionalWith<typeof S.String, {
        nullable: true;
    }>;
    temperature: S.optionalWith<typeof S.Number, {
        nullable: true;
    }>;
    max_response_output_tokens: S.optionalWith<S.Union<[typeof S.Int, typeof RealtimeSessionCreateResponseMaxResponseOutputTokensEnum]>, {
        nullable: true;
    }>;
}>, never, {
    readonly instructions?: string | undefined;
} & {
    readonly tools?: readonly {
        readonly description?: string | undefined;
        readonly type?: "function" | undefined;
        readonly name?: string | undefined;
        readonly parameters?: {
            readonly [x: string]: unknown;
        } | undefined;
    }[] | undefined;
} & {
    readonly temperature?: number | undefined;
} & {
    readonly voice?: string | undefined;
} & {
    readonly tool_choice?: string | undefined;
} & {
    readonly input_audio_format?: string | undefined;
} & {
    readonly output_audio_format?: string | undefined;
} & {
    readonly input_audio_transcription?: {
        readonly model?: string | undefined;
    } | undefined;
} & {
    readonly turn_detection?: {
        readonly type?: string | undefined;
        readonly threshold?: number | undefined;
        readonly prefix_padding_ms?: number | undefined;
        readonly silence_duration_ms?: number | undefined;
    } | undefined;
} & {
    readonly max_response_output_tokens?: number | "inf" | undefined;
} & {
    readonly client_secret: {
        readonly value: string;
        readonly expires_at: number;
    };
}, {}, {}>;
export declare class RealtimeSessionCreateResponse extends RealtimeSessionCreateResponse_base {
}
declare const RealtimeTranscriptionSessionCreateRequestInputAudioFormat_base: S.Literal<["pcm16", "g711_ulaw", "g711_alaw"]>;
export declare class RealtimeTranscriptionSessionCreateRequestInputAudioFormat extends RealtimeTranscriptionSessionCreateRequestInputAudioFormat_base {
}
declare const RealtimeTranscriptionSessionCreateRequestInputAudioTranscriptionModel_base: S.Literal<["gpt-4o-transcribe", "gpt-4o-mini-transcribe", "whisper-1"]>;
export declare class RealtimeTranscriptionSessionCreateRequestInputAudioTranscriptionModel extends RealtimeTranscriptionSessionCreateRequestInputAudioTranscriptionModel_base {
}
declare const RealtimeTranscriptionSessionCreateRequestTurnDetectionType_base: S.Literal<["server_vad", "semantic_vad"]>;
export declare class RealtimeTranscriptionSessionCreateRequestTurnDetectionType extends RealtimeTranscriptionSessionCreateRequestTurnDetectionType_base {
}
declare const RealtimeTranscriptionSessionCreateRequestTurnDetectionEagerness_base: S.Literal<["low", "medium", "high", "auto"]>;
export declare class RealtimeTranscriptionSessionCreateRequestTurnDetectionEagerness extends RealtimeTranscriptionSessionCreateRequestTurnDetectionEagerness_base {
}
declare const RealtimeTranscriptionSessionCreateRequestInputAudioNoiseReductionType_base: S.Literal<["near_field", "far_field"]>;
export declare class RealtimeTranscriptionSessionCreateRequestInputAudioNoiseReductionType extends RealtimeTranscriptionSessionCreateRequestInputAudioNoiseReductionType_base {
}
declare const RealtimeTranscriptionSessionCreateRequest_base: S.Class<RealtimeTranscriptionSessionCreateRequest, {
    input_audio_format: S.optionalWith<typeof RealtimeTranscriptionSessionCreateRequestInputAudioFormat, {
        nullable: true;
        default: () => "pcm16";
    }>;
    input_audio_transcription: S.optionalWith<S.Struct<{
        model: S.optionalWith<typeof RealtimeTranscriptionSessionCreateRequestInputAudioTranscriptionModel, {
            nullable: true;
        }>;
        language: S.optionalWith<typeof S.String, {
            nullable: true;
        }>;
        prompt: S.optionalWith<typeof S.String, {
            nullable: true;
        }>;
    }>, {
        nullable: true;
    }>;
    turn_detection: S.optionalWith<S.Struct<{
        type: S.optionalWith<typeof RealtimeTranscriptionSessionCreateRequestTurnDetectionType, {
            nullable: true;
            default: () => "server_vad";
        }>;
        eagerness: S.optionalWith<typeof RealtimeTranscriptionSessionCreateRequestTurnDetectionEagerness, {
            nullable: true;
            default: () => "auto";
        }>;
        threshold: S.optionalWith<typeof S.Number, {
            nullable: true;
        }>;
        prefix_padding_ms: S.optionalWith<typeof S.Int, {
            nullable: true;
        }>;
        silence_duration_ms: S.optionalWith<typeof S.Int, {
            nullable: true;
        }>;
        create_response: S.optionalWith<typeof S.Boolean, {
            nullable: true;
            default: () => true;
        }>;
        interrupt_response: S.optionalWith<typeof S.Boolean, {
            nullable: true;
            default: () => true;
        }>;
    }>, {
        nullable: true;
    }>;
    input_audio_noise_reduction: S.optionalWith<S.NullOr<S.Struct<{
        type: S.optionalWith<typeof RealtimeTranscriptionSessionCreateRequestInputAudioNoiseReductionType, {
            nullable: true;
        }>;
    }>>, {
        default: () => null;
    }>;
    include: S.optionalWith<S.Array$<typeof S.String>, {
        nullable: true;
    }>;
}, S.Struct.Encoded<{
    input_audio_format: S.optionalWith<typeof RealtimeTranscriptionSessionCreateRequestInputAudioFormat, {
        nullable: true;
        default: () => "pcm16";
    }>;
    input_audio_transcription: S.optionalWith<S.Struct<{
        model: S.optionalWith<typeof RealtimeTranscriptionSessionCreateRequestInputAudioTranscriptionModel, {
            nullable: true;
        }>;
        language: S.optionalWith<typeof S.String, {
            nullable: true;
        }>;
        prompt: S.optionalWith<typeof S.String, {
            nullable: true;
        }>;
    }>, {
        nullable: true;
    }>;
    turn_detection: S.optionalWith<S.Struct<{
        type: S.optionalWith<typeof RealtimeTranscriptionSessionCreateRequestTurnDetectionType, {
            nullable: true;
            default: () => "server_vad";
        }>;
        eagerness: S.optionalWith<typeof RealtimeTranscriptionSessionCreateRequestTurnDetectionEagerness, {
            nullable: true;
            default: () => "auto";
        }>;
        threshold: S.optionalWith<typeof S.Number, {
            nullable: true;
        }>;
        prefix_padding_ms: S.optionalWith<typeof S.Int, {
            nullable: true;
        }>;
        silence_duration_ms: S.optionalWith<typeof S.Int, {
            nullable: true;
        }>;
        create_response: S.optionalWith<typeof S.Boolean, {
            nullable: true;
            default: () => true;
        }>;
        interrupt_response: S.optionalWith<typeof S.Boolean, {
            nullable: true;
            default: () => true;
        }>;
    }>, {
        nullable: true;
    }>;
    input_audio_noise_reduction: S.optionalWith<S.NullOr<S.Struct<{
        type: S.optionalWith<typeof RealtimeTranscriptionSessionCreateRequestInputAudioNoiseReductionType, {
            nullable: true;
        }>;
    }>>, {
        default: () => null;
    }>;
    include: S.optionalWith<S.Array$<typeof S.String>, {
        nullable: true;
    }>;
}>, never, {
    readonly input_audio_format?: "pcm16" | "g711_ulaw" | "g711_alaw";
} & {
    readonly input_audio_transcription?: {
        readonly model?: "gpt-4o-transcribe" | "gpt-4o-mini-transcribe" | "whisper-1" | undefined;
        readonly language?: string | undefined;
        readonly prompt?: string | undefined;
    } | undefined;
} & {
    readonly turn_detection?: {
        readonly type: "server_vad" | "semantic_vad";
        readonly eagerness: "auto" | "low" | "medium" | "high";
        readonly threshold?: number | undefined;
        readonly prefix_padding_ms?: number | undefined;
        readonly silence_duration_ms?: number | undefined;
        readonly create_response: boolean;
        readonly interrupt_response: boolean;
    } | undefined;
} & {
    readonly input_audio_noise_reduction?: {
        readonly type?: "near_field" | "far_field" | undefined;
    } | null;
} & {
    readonly include?: readonly string[] | undefined;
}, {}, {}>;
export declare class RealtimeTranscriptionSessionCreateRequest extends RealtimeTranscriptionSessionCreateRequest_base {
}
declare const RealtimeTranscriptionSessionCreateResponseInputAudioTranscriptionModel_base: S.Literal<["gpt-4o-transcribe", "gpt-4o-mini-transcribe", "whisper-1"]>;
export declare class RealtimeTranscriptionSessionCreateResponseInputAudioTranscriptionModel extends RealtimeTranscriptionSessionCreateResponseInputAudioTranscriptionModel_base {
}
declare const RealtimeTranscriptionSessionCreateResponse_base: S.Class<RealtimeTranscriptionSessionCreateResponse, {
    client_secret: S.Struct<{
        value: typeof S.String;
        expires_at: typeof S.Int;
    }>;
    input_audio_format: S.optionalWith<typeof S.String, {
        nullable: true;
    }>;
    input_audio_transcription: S.optionalWith<S.Struct<{
        model: S.optionalWith<typeof RealtimeTranscriptionSessionCreateResponseInputAudioTranscriptionModel, {
            nullable: true;
        }>;
        language: S.optionalWith<typeof S.String, {
            nullable: true;
        }>;
        prompt: S.optionalWith<typeof S.String, {
            nullable: true;
        }>;
    }>, {
        nullable: true;
    }>;
    turn_detection: S.optionalWith<S.Struct<{
        type: S.optionalWith<typeof S.String, {
            nullable: true;
        }>;
        threshold: S.optionalWith<typeof S.Number, {
            nullable: true;
        }>;
        prefix_padding_ms: S.optionalWith<typeof S.Int, {
            nullable: true;
        }>;
        silence_duration_ms: S.optionalWith<typeof S.Int, {
            nullable: true;
        }>;
    }>, {
        nullable: true;
    }>;
}, S.Struct.Encoded<{
    client_secret: S.Struct<{
        value: typeof S.String;
        expires_at: typeof S.Int;
    }>;
    input_audio_format: S.optionalWith<typeof S.String, {
        nullable: true;
    }>;
    input_audio_transcription: S.optionalWith<S.Struct<{
        model: S.optionalWith<typeof RealtimeTranscriptionSessionCreateResponseInputAudioTranscriptionModel, {
            nullable: true;
        }>;
        language: S.optionalWith<typeof S.String, {
            nullable: true;
        }>;
        prompt: S.optionalWith<typeof S.String, {
            nullable: true;
        }>;
    }>, {
        nullable: true;
    }>;
    turn_detection: S.optionalWith<S.Struct<{
        type: S.optionalWith<typeof S.String, {
            nullable: true;
        }>;
        threshold: S.optionalWith<typeof S.Number, {
            nullable: true;
        }>;
        prefix_padding_ms: S.optionalWith<typeof S.Int, {
            nullable: true;
        }>;
        silence_duration_ms: S.optionalWith<typeof S.Int, {
            nullable: true;
        }>;
    }>, {
        nullable: true;
    }>;
}>, never, {
    readonly input_audio_format?: string | undefined;
} & {
    readonly input_audio_transcription?: {
        readonly model?: "gpt-4o-transcribe" | "gpt-4o-mini-transcribe" | "whisper-1" | undefined;
        readonly language?: string | undefined;
        readonly prompt?: string | undefined;
    } | undefined;
} & {
    readonly turn_detection?: {
        readonly type?: string | undefined;
        readonly threshold?: number | undefined;
        readonly prefix_padding_ms?: number | undefined;
        readonly silence_duration_ms?: number | undefined;
    } | undefined;
} & {
    readonly client_secret: {
        readonly value: string;
        readonly expires_at: number;
    };
}, {}, {}>;
export declare class RealtimeTranscriptionSessionCreateResponse extends RealtimeTranscriptionSessionCreateResponse_base {
}
declare const EasyInputMessageRole_base: S.Literal<["user", "assistant", "system", "developer"]>;
export declare class EasyInputMessageRole extends EasyInputMessageRole_base {
}
declare const InputTextType_base: S.Literal<["input_text"]>;
export declare class InputTextType extends InputTextType_base {
}
declare const InputText_base: S.Struct<{
    type: typeof InputTextType;
    text: typeof S.String;
}>;
export declare class InputText extends InputText_base {
}
declare const InputImageType_base: S.Literal<["input_image"]>;
export declare class InputImageType extends InputImageType_base {
}
declare const InputImageDetail_base: S.Literal<["high", "low", "auto"]>;
export declare class InputImageDetail extends InputImageDetail_base {
}
declare const InputImage_base: S.Struct<{
    type: typeof InputImageType;
    image_url: S.optionalWith<typeof S.String, {
        nullable: true;
    }>;
    file_id: S.optionalWith<typeof S.String, {
        nullable: true;
    }>;
    detail: S.PropertySignature<":", "auto" | "low" | "high", never, ":", "auto" | "low" | "high", true, never>;
}>;
export declare class InputImage extends InputImage_base {
}
declare const InputFileType_base: S.Literal<["input_file"]>;
export declare class InputFileType extends InputFileType_base {
}
declare const InputFile_base: S.Struct<{
    type: typeof InputFileType;
    file_id: S.optionalWith<typeof S.String, {
        nullable: true;
    }>;
    filename: S.optionalWith<typeof S.String, {
        nullable: true;
    }>;
    file_data: S.optionalWith<typeof S.String, {
        nullable: true;
    }>;
}>;
export declare class InputFile extends InputFile_base {
}
declare const InputContent_base: S.Union<[typeof InputText, typeof InputImage, typeof InputFile]>;
export declare class InputContent extends InputContent_base {
}
declare const InputMessageContentList_base: S.Array$<typeof InputContent>;
export declare class InputMessageContentList extends InputMessageContentList_base {
}
declare const EasyInputMessageType_base: S.Literal<["message"]>;
export declare class EasyInputMessageType extends EasyInputMessageType_base {
}
declare const EasyInputMessage_base: S.Struct<{
    role: typeof EasyInputMessageRole;
    content: S.Union<[typeof S.String, typeof InputMessageContentList]>;
    type: S.optionalWith<typeof EasyInputMessageType, {
        nullable: true;
    }>;
}>;
export declare class EasyInputMessage extends EasyInputMessage_base {
}
declare const InputMessageType_base: S.Literal<["message"]>;
export declare class InputMessageType extends InputMessageType_base {
}
declare const InputMessageRole_base: S.Literal<["user", "system", "developer"]>;
export declare class InputMessageRole extends InputMessageRole_base {
}
declare const InputMessageStatus_base: S.Literal<["in_progress", "completed", "incomplete"]>;
export declare class InputMessageStatus extends InputMessageStatus_base {
}
declare const InputMessage_base: S.Struct<{
    type: S.optionalWith<typeof InputMessageType, {
        nullable: true;
    }>;
    role: typeof InputMessageRole;
    status: S.optionalWith<typeof InputMessageStatus, {
        nullable: true;
    }>;
    content: typeof InputMessageContentList;
}>;
export declare class InputMessage extends InputMessage_base {
}
declare const OutputMessageType_base: S.Literal<["message"]>;
export declare class OutputMessageType extends OutputMessageType_base {
}
declare const OutputMessageRole_base: S.Literal<["assistant"]>;
export declare class OutputMessageRole extends OutputMessageRole_base {
}
declare const OutputTextType_base: S.Literal<["output_text"]>;
export declare class OutputTextType extends OutputTextType_base {
}
declare const FileCitationType_base: S.Literal<["file_citation"]>;
export declare class FileCitationType extends FileCitationType_base {
}
declare const FileCitation_base: S.Struct<{
    type: typeof FileCitationType;
    index: typeof S.Int;
    file_id: typeof S.String;
}>;
export declare class FileCitation extends FileCitation_base {
}
declare const UrlCitationType_base: S.Literal<["url_citation"]>;
export declare class UrlCitationType extends UrlCitationType_base {
}
declare const UrlCitation_base: S.Struct<{
    url: typeof S.String;
    title: typeof S.String;
    type: typeof UrlCitationType;
    start_index: typeof S.Int;
    end_index: typeof S.Int;
}>;
export declare class UrlCitation extends UrlCitation_base {
}
declare const FilePathType_base: S.Literal<["file_path"]>;
export declare class FilePathType extends FilePathType_base {
}
declare const FilePath_base: S.Struct<{
    type: typeof FilePathType;
    file_id: typeof S.String;
    index: typeof S.Int;
}>;
export declare class FilePath extends FilePath_base {
}
declare const Annotation_base: S.Union<[typeof FileCitation, typeof UrlCitation, typeof FilePath]>;
export declare class Annotation extends Annotation_base {
}
declare const OutputText_base: S.Struct<{
    type: typeof OutputTextType;
    text: typeof S.String;
    annotations: S.Array$<typeof Annotation>;
}>;
export declare class OutputText extends OutputText_base {
}
declare const RefusalType_base: S.Literal<["refusal"]>;
export declare class RefusalType extends RefusalType_base {
}
declare const Refusal_base: S.Struct<{
    type: typeof RefusalType;
    refusal: typeof S.String;
}>;
export declare class Refusal extends Refusal_base {
}
declare const OutputContent_base: S.Union<[typeof OutputText, typeof Refusal]>;
export declare class OutputContent extends OutputContent_base {
}
declare const OutputMessageStatus_base: S.Literal<["in_progress", "completed", "incomplete"]>;
export declare class OutputMessageStatus extends OutputMessageStatus_base {
}
declare const OutputMessage_base: S.Struct<{
    id: typeof S.String;
    type: typeof OutputMessageType;
    role: typeof OutputMessageRole;
    content: S.Array$<typeof OutputContent>;
    status: typeof OutputMessageStatus;
}>;
export declare class OutputMessage extends OutputMessage_base {
}
declare const FileSearchToolCallType_base: S.Literal<["file_search_call"]>;
export declare class FileSearchToolCallType extends FileSearchToolCallType_base {
}
declare const FileSearchToolCallStatus_base: S.Literal<["in_progress", "searching", "completed", "incomplete", "failed"]>;
export declare class FileSearchToolCallStatus extends FileSearchToolCallStatus_base {
}
declare const VectorStoreFileAttributes_base: S.Record$<typeof S.String, typeof S.Unknown>;
export declare class VectorStoreFileAttributes extends VectorStoreFileAttributes_base {
}
declare const FileSearchToolCall_base: S.Struct<{
    id: typeof S.String;
    type: typeof FileSearchToolCallType;
    status: typeof FileSearchToolCallStatus;
    queries: S.Array$<typeof S.String>;
    results: S.optionalWith<S.Array$<S.Struct<{
        file_id: S.optionalWith<typeof S.String, {
            nullable: true;
        }>;
        text: S.optionalWith<typeof S.String, {
            nullable: true;
        }>;
        filename: S.optionalWith<typeof S.String, {
            nullable: true;
        }>;
        attributes: S.optionalWith<typeof VectorStoreFileAttributes, {
            nullable: true;
        }>;
        score: S.optionalWith<typeof S.Number, {
            nullable: true;
        }>;
    }>>, {
        nullable: true;
    }>;
}>;
export declare class FileSearchToolCall extends FileSearchToolCall_base {
}
declare const ComputerToolCallType_base: S.Literal<["computer_call"]>;
export declare class ComputerToolCallType extends ComputerToolCallType_base {
}
declare const ClickType_base: S.Literal<["click"]>;
export declare class ClickType extends ClickType_base {
}
declare const ClickButton_base: S.Literal<["left", "right", "wheel", "back", "forward"]>;
export declare class ClickButton extends ClickButton_base {
}
declare const Click_base: S.Struct<{
    type: S.PropertySignature<":", "click", never, ":", "click", true, never>;
    button: typeof ClickButton;
    x: typeof S.Int;
    y: typeof S.Int;
}>;
export declare class Click extends Click_base {
}
declare const DoubleClickType_base: S.Literal<["double_click"]>;
export declare class DoubleClickType extends DoubleClickType_base {
}
declare const DoubleClick_base: S.Struct<{
    type: S.PropertySignature<":", "double_click", never, ":", "double_click", true, never>;
    x: typeof S.Int;
    y: typeof S.Int;
}>;
export declare class DoubleClick extends DoubleClick_base {
}
declare const DragType_base: S.Literal<["drag"]>;
export declare class DragType extends DragType_base {
}
declare const Coordinate_base: S.Struct<{
    x: typeof S.Int;
    y: typeof S.Int;
}>;
export declare class Coordinate extends Coordinate_base {
}
declare const Drag_base: S.Struct<{
    type: S.PropertySignature<":", "drag", never, ":", "drag", true, never>;
    path: S.Array$<typeof Coordinate>;
}>;
export declare class Drag extends Drag_base {
}
declare const KeyPressType_base: S.Literal<["keypress"]>;
export declare class KeyPressType extends KeyPressType_base {
}
declare const KeyPress_base: S.Struct<{
    type: S.PropertySignature<":", "keypress", never, ":", "keypress", true, never>;
    keys: S.Array$<typeof S.String>;
}>;
export declare class KeyPress extends KeyPress_base {
}
declare const MoveType_base: S.Literal<["move"]>;
export declare class MoveType extends MoveType_base {
}
declare const Move_base: S.Struct<{
    type: S.PropertySignature<":", "move", never, ":", "move", true, never>;
    x: typeof S.Int;
    y: typeof S.Int;
}>;
export declare class Move extends Move_base {
}
declare const ScreenshotType_base: S.Literal<["screenshot"]>;
export declare class ScreenshotType extends ScreenshotType_base {
}
declare const Screenshot_base: S.Struct<{
    type: S.PropertySignature<":", "screenshot", never, ":", "screenshot", true, never>;
}>;
export declare class Screenshot extends Screenshot_base {
}
declare const ScrollType_base: S.Literal<["scroll"]>;
export declare class ScrollType extends ScrollType_base {
}
declare const Scroll_base: S.Struct<{
    type: S.PropertySignature<":", "scroll", never, ":", "scroll", true, never>;
    x: typeof S.Int;
    y: typeof S.Int;
    scroll_x: typeof S.Int;
    scroll_y: typeof S.Int;
}>;
export declare class Scroll extends Scroll_base {
}
declare const TypeType_base: S.Literal<["type"]>;
export declare class TypeType extends TypeType_base {
}
declare const Type_base: S.Struct<{
    type: S.PropertySignature<":", "type", never, ":", "type", true, never>;
    text: typeof S.String;
}>;
export declare class Type extends Type_base {
}
declare const WaitType_base: S.Literal<["wait"]>;
export declare class WaitType extends WaitType_base {
}
declare const Wait_base: S.Struct<{
    type: S.PropertySignature<":", "wait", never, ":", "wait", true, never>;
}>;
export declare class Wait extends Wait_base {
}
declare const ComputerAction_base: S.Union<[typeof Click, typeof DoubleClick, typeof Drag, typeof KeyPress, typeof Move, typeof Screenshot, typeof Scroll, typeof Type, typeof Wait]>;
export declare class ComputerAction extends ComputerAction_base {
}
declare const ComputerToolCallSafetyCheck_base: S.Struct<{
    id: typeof S.String;
    code: typeof S.String;
    message: typeof S.String;
}>;
export declare class ComputerToolCallSafetyCheck extends ComputerToolCallSafetyCheck_base {
}
declare const ComputerToolCallStatus_base: S.Literal<["in_progress", "completed", "incomplete"]>;
export declare class ComputerToolCallStatus extends ComputerToolCallStatus_base {
}
declare const ComputerToolCall_base: S.Struct<{
    type: S.PropertySignature<":", "computer_call", never, ":", "computer_call", true, never>;
    id: typeof S.String;
    call_id: typeof S.String;
    action: typeof ComputerAction;
    pending_safety_checks: S.Array$<typeof ComputerToolCallSafetyCheck>;
    status: typeof ComputerToolCallStatus;
}>;
export declare class ComputerToolCall extends ComputerToolCall_base {
}
declare const ComputerToolCallOutputType_base: S.Literal<["computer_call_output"]>;
export declare class ComputerToolCallOutputType extends ComputerToolCallOutputType_base {
}
declare const ComputerScreenshotImageType_base: S.Literal<["computer_screenshot"]>;
export declare class ComputerScreenshotImageType extends ComputerScreenshotImageType_base {
}
declare const ComputerScreenshotImage_base: S.Struct<{
    type: S.PropertySignature<":", "computer_screenshot", never, ":", "computer_screenshot", true, never>;
    image_url: S.optionalWith<typeof S.String, {
        nullable: true;
    }>;
    file_id: S.optionalWith<typeof S.String, {
        nullable: true;
    }>;
}>;
export declare class ComputerScreenshotImage extends ComputerScreenshotImage_base {
}
declare const ComputerToolCallOutputStatus_base: S.Literal<["in_progress", "completed", "incomplete"]>;
export declare class ComputerToolCallOutputStatus extends ComputerToolCallOutputStatus_base {
}
declare const ComputerToolCallOutput_base: S.Struct<{
    type: S.PropertySignature<":", "computer_call_output", never, ":", "computer_call_output", true, never>;
    id: S.optionalWith<typeof S.String, {
        nullable: true;
    }>;
    call_id: typeof S.String;
    acknowledged_safety_checks: S.optionalWith<S.Array$<typeof ComputerToolCallSafetyCheck>, {
        nullable: true;
    }>;
    output: typeof ComputerScreenshotImage;
    status: S.optionalWith<typeof ComputerToolCallOutputStatus, {
        nullable: true;
    }>;
}>;
export declare class ComputerToolCallOutput extends ComputerToolCallOutput_base {
}
declare const WebSearchToolCallType_base: S.Literal<["web_search_call"]>;
export declare class WebSearchToolCallType extends WebSearchToolCallType_base {
}
declare const WebSearchToolCallStatus_base: S.Literal<["in_progress", "searching", "completed", "failed"]>;
export declare class WebSearchToolCallStatus extends WebSearchToolCallStatus_base {
}
declare const WebSearchToolCall_base: S.Struct<{
    id: typeof S.String;
    type: typeof WebSearchToolCallType;
    status: typeof WebSearchToolCallStatus;
}>;
export declare class WebSearchToolCall extends WebSearchToolCall_base {
}
declare const FunctionToolCallType_base: S.Literal<["function_call"]>;
export declare class FunctionToolCallType extends FunctionToolCallType_base {
}
declare const FunctionToolCallStatus_base: S.Literal<["in_progress", "completed", "incomplete"]>;
export declare class FunctionToolCallStatus extends FunctionToolCallStatus_base {
}
declare const FunctionToolCall_base: S.Struct<{
    id: S.optionalWith<typeof S.String, {
        nullable: true;
    }>;
    type: typeof FunctionToolCallType;
    call_id: typeof S.String;
    name: typeof S.String;
    arguments: typeof S.String;
    status: S.optionalWith<typeof FunctionToolCallStatus, {
        nullable: true;
    }>;
}>;
export declare class FunctionToolCall extends FunctionToolCall_base {
}
declare const FunctionToolCallOutputType_base: S.Literal<["function_call_output"]>;
export declare class FunctionToolCallOutputType extends FunctionToolCallOutputType_base {
}
declare const FunctionToolCallOutputStatus_base: S.Literal<["in_progress", "completed", "incomplete"]>;
export declare class FunctionToolCallOutputStatus extends FunctionToolCallOutputStatus_base {
}
declare const FunctionToolCallOutput_base: S.Struct<{
    id: S.optionalWith<typeof S.String, {
        nullable: true;
    }>;
    type: typeof FunctionToolCallOutputType;
    call_id: typeof S.String;
    output: typeof S.String;
    status: S.optionalWith<typeof FunctionToolCallOutputStatus, {
        nullable: true;
    }>;
}>;
export declare class FunctionToolCallOutput extends FunctionToolCallOutput_base {
}
declare const ReasoningItemType_base: S.Literal<["reasoning"]>;
export declare class ReasoningItemType extends ReasoningItemType_base {
}
declare const ReasoningItemStatus_base: S.Literal<["in_progress", "completed", "incomplete"]>;
export declare class ReasoningItemStatus extends ReasoningItemStatus_base {
}
declare const ReasoningItem_base: S.Struct<{
    type: typeof ReasoningItemType;
    id: typeof S.String;
    summary: S.Array$<S.Struct<{
        type: S.Literal<["summary_text"]>;
        text: typeof S.String;
    }>>;
    status: S.optionalWith<typeof ReasoningItemStatus, {
        nullable: true;
    }>;
}>;
export declare class ReasoningItem extends ReasoningItem_base {
}
declare const Item_base: S.Record$<typeof S.String, typeof S.Unknown>;
export declare class Item extends Item_base {
}
declare const ItemReferenceType_base: S.Literal<["item_reference"]>;
export declare class ItemReferenceType extends ItemReferenceType_base {
}
declare const ItemReference_base: S.Struct<{
    id: typeof S.String;
    type: typeof ItemReferenceType;
}>;
export declare class ItemReference extends ItemReference_base {
}
declare const InputItem_base: S.Union<[typeof EasyInputMessage, S.Record$<typeof S.String, typeof S.Unknown>, typeof ItemReference]>;
export declare class InputItem extends InputItem_base {
}
declare const Includable_base: S.Literal<["file_search_call.results", "message.input_image.image_url", "computer_call_output.output.image_url"]>;
export declare class Includable extends Includable_base {
}
declare const ModelIdsResponsesEnum_base: S.Literal<["o1-pro", "o1-pro-2025-03-19", "computer-use-preview", "computer-use-preview-2025-03-11"]>;
export declare class ModelIdsResponsesEnum extends ModelIdsResponsesEnum_base {
}
declare const ModelIdsResponses_base: S.Union<[typeof ModelIdsShared, typeof ModelIdsResponsesEnum]>;
export declare class ModelIdsResponses extends ModelIdsResponses_base {
}
declare const ReasoningGenerateSummary_base: S.Literal<["concise", "detailed"]>;
export declare class ReasoningGenerateSummary extends ReasoningGenerateSummary_base {
}
declare const Reasoning_base: S.Struct<{
    effort: S.optionalWith<typeof ReasoningEffort, {
        nullable: true;
        default: () => "medium";
    }>;
    generate_summary: S.optionalWith<typeof ReasoningGenerateSummary, {
        nullable: true;
    }>;
}>;
export declare class Reasoning extends Reasoning_base {
}
declare const TextResponseFormatJsonSchemaType_base: S.Literal<["json_schema"]>;
export declare class TextResponseFormatJsonSchemaType extends TextResponseFormatJsonSchemaType_base {
}
declare const TextResponseFormatJsonSchema_base: S.Struct<{
    type: typeof TextResponseFormatJsonSchemaType;
    description: S.optionalWith<typeof S.String, {
        nullable: true;
    }>;
    name: S.optionalWith<typeof S.String, {
        nullable: true;
    }>;
    schema: typeof ResponseFormatJsonSchemaSchema;
    strict: S.optionalWith<typeof S.Boolean, {
        nullable: true;
        default: () => false;
    }>;
}>;
export declare class TextResponseFormatJsonSchema extends TextResponseFormatJsonSchema_base {
}
declare const TextResponseFormatConfiguration_base: S.Union<[typeof ResponseFormatText, typeof TextResponseFormatJsonSchema, typeof ResponseFormatJsonObject]>;
export declare class TextResponseFormatConfiguration extends TextResponseFormatConfiguration_base {
}
declare const FileSearchToolType_base: S.Literal<["file_search"]>;
export declare class FileSearchToolType extends FileSearchToolType_base {
}
declare const ComparisonFilterType_base: S.Literal<["eq", "ne", "gt", "gte", "lt", "lte"]>;
export declare class ComparisonFilterType extends ComparisonFilterType_base {
}
declare const ComparisonFilter_base: S.Struct<{
    type: S.PropertySignature<":", "eq" | "ne" | "gt" | "gte" | "lt" | "lte", never, ":", "eq" | "ne" | "gt" | "gte" | "lt" | "lte", true, never>;
    key: typeof S.String;
    value: S.Union<[typeof S.String, typeof S.Number, typeof S.Boolean]>;
}>;
export declare class ComparisonFilter extends ComparisonFilter_base {
}
declare const CompoundFilterType_base: S.Literal<["and", "or"]>;
export declare class CompoundFilterType extends CompoundFilterType_base {
}
declare const CompoundFilter_base: S.Struct<{
    type: typeof CompoundFilterType;
    filters: S.Array$<S.Union<[typeof ComparisonFilter, S.Record$<typeof S.String, typeof S.Unknown>]>>;
}>;
export declare class CompoundFilter extends CompoundFilter_base {
}
declare const FileSearchToolRankingOptionsRanker_base: S.Literal<["auto", "default-2024-11-15"]>;
export declare class FileSearchToolRankingOptionsRanker extends FileSearchToolRankingOptionsRanker_base {
}
declare const FileSearchTool_base: S.Struct<{
    type: typeof FileSearchToolType;
    vector_store_ids: S.Array$<typeof S.String>;
    max_num_results: S.optionalWith<typeof S.Int, {
        nullable: true;
    }>;
    filters: S.optionalWith<S.Union<[typeof ComparisonFilter, typeof CompoundFilter]>, {
        nullable: true;
    }>;
    ranking_options: S.optionalWith<S.Struct<{
        ranker: S.optionalWith<typeof FileSearchToolRankingOptionsRanker, {
            nullable: true;
            default: () => "auto";
        }>;
        score_threshold: S.optionalWith<S.filter<S.filter<typeof S.Number>>, {
            nullable: true;
            default: () => 0;
        }>;
    }>, {
        nullable: true;
    }>;
}>;
export declare class FileSearchTool extends FileSearchTool_base {
}
declare const FunctionToolType_base: S.Literal<["function"]>;
export declare class FunctionToolType extends FunctionToolType_base {
}
declare const FunctionTool_base: S.Struct<{
    type: typeof FunctionToolType;
    name: typeof S.String;
    description: S.optionalWith<typeof S.String, {
        nullable: true;
    }>;
    parameters: S.Record$<typeof S.String, typeof S.Unknown>;
    strict: typeof S.Boolean;
}>;
export declare class FunctionTool extends FunctionTool_base {
}
declare const ComputerToolType_base: S.Literal<["computer_use_preview"]>;
export declare class ComputerToolType extends ComputerToolType_base {
}
declare const ComputerToolEnvironment_base: S.Literal<["mac", "windows", "ubuntu", "browser"]>;
export declare class ComputerToolEnvironment extends ComputerToolEnvironment_base {
}
declare const ComputerTool_base: S.Struct<{
    type: typeof ComputerToolType;
    display_width: typeof S.Number;
    display_height: typeof S.Number;
    environment: typeof ComputerToolEnvironment;
}>;
export declare class ComputerTool extends ComputerTool_base {
}
declare const WebSearchToolType_base: S.Literal<["web_search_preview", "web_search_preview_2025_03_11"]>;
export declare class WebSearchToolType extends WebSearchToolType_base {
}
declare const WebSearchToolUserLocationEnumType_base: S.Literal<["approximate"]>;
export declare class WebSearchToolUserLocationEnumType extends WebSearchToolUserLocationEnumType_base {
}
declare const WebSearchToolUserLocation_base: S.Struct<{
    type: S.Literal<["approximate"]>;
    country: S.optionalWith<typeof S.String, {
        nullable: true;
    }>;
    region: S.optionalWith<typeof S.String, {
        nullable: true;
    }>;
    city: S.optionalWith<typeof S.String, {
        nullable: true;
    }>;
    timezone: S.optionalWith<typeof S.String, {
        nullable: true;
    }>;
}>;
export declare class WebSearchToolUserLocation extends WebSearchToolUserLocation_base {
}
declare const WebSearchTool_base: S.Struct<{
    type: typeof WebSearchToolType;
    user_location: S.optionalWith<typeof WebSearchToolUserLocation, {
        nullable: true;
    }>;
    search_context_size: S.optionalWith<typeof WebSearchContextSize, {
        nullable: true;
        default: () => "medium";
    }>;
}>;
export declare class WebSearchTool extends WebSearchTool_base {
}
declare const Tool_base: S.Union<[typeof FileSearchTool, typeof FunctionTool, typeof ComputerTool, typeof WebSearchTool]>;
export declare class Tool extends Tool_base {
}
declare const ToolChoiceOptions_base: S.Literal<["none", "auto", "required"]>;
export declare class ToolChoiceOptions extends ToolChoiceOptions_base {
}
declare const ToolChoiceTypesType_base: S.Literal<["file_search", "web_search_preview", "computer_use_preview", "web_search_preview_2025_03_11"]>;
export declare class ToolChoiceTypesType extends ToolChoiceTypesType_base {
}
declare const ToolChoiceTypes_base: S.Struct<{
    type: typeof ToolChoiceTypesType;
}>;
export declare class ToolChoiceTypes extends ToolChoiceTypes_base {
}
declare const ToolChoiceFunctionType_base: S.Literal<["function"]>;
export declare class ToolChoiceFunctionType extends ToolChoiceFunctionType_base {
}
declare const ToolChoiceFunction_base: S.Struct<{
    type: typeof ToolChoiceFunctionType;
    name: typeof S.String;
}>;
export declare class ToolChoiceFunction extends ToolChoiceFunction_base {
}
declare const CreateResponseTruncation_base: S.Literal<["auto", "disabled"]>;
export declare class CreateResponseTruncation extends CreateResponseTruncation_base {
}
declare const CreateResponse_base: S.Class<CreateResponse, {
    input: S.Union<[typeof S.String, S.Array$<typeof InputItem>]>;
    include: S.optionalWith<S.Array$<typeof Includable>, {
        nullable: true;
    }>;
    parallel_tool_calls: S.optionalWith<typeof S.Boolean, {
        nullable: true;
        default: () => true;
    }>;
    store: S.optionalWith<typeof S.Boolean, {
        nullable: true;
        default: () => true;
    }>;
    stream: S.optionalWith<typeof S.Boolean, {
        nullable: true;
        default: () => false;
    }>;
    previous_response_id: S.optionalWith<typeof S.String, {
        nullable: true;
    }>;
    model: typeof ModelIdsResponses;
    reasoning: S.optionalWith<typeof Reasoning, {
        nullable: true;
    }>;
    max_output_tokens: S.optionalWith<typeof S.Int, {
        nullable: true;
    }>;
    instructions: S.optionalWith<typeof S.String, {
        nullable: true;
    }>;
    text: S.optionalWith<S.Struct<{
        format: S.optionalWith<typeof TextResponseFormatConfiguration, {
            nullable: true;
        }>;
    }>, {
        nullable: true;
    }>;
    tools: S.optionalWith<S.Array$<typeof Tool>, {
        nullable: true;
    }>;
    tool_choice: S.optionalWith<S.Union<[typeof ToolChoiceOptions, typeof ToolChoiceTypes, typeof ToolChoiceFunction]>, {
        nullable: true;
    }>;
    truncation: S.optionalWith<typeof CreateResponseTruncation, {
        nullable: true;
        default: () => "disabled";
    }>;
    metadata: S.optionalWith<typeof Metadata, {
        nullable: true;
    }>;
    temperature: S.optionalWith<S.filter<S.filter<typeof S.Number>>, {
        nullable: true;
        default: () => 1;
    }>;
    top_p: S.optionalWith<S.filter<S.filter<typeof S.Number>>, {
        nullable: true;
        default: () => 1;
    }>;
    user: S.optionalWith<typeof S.String, {
        nullable: true;
    }>;
}, S.Struct.Encoded<{
    input: S.Union<[typeof S.String, S.Array$<typeof InputItem>]>;
    include: S.optionalWith<S.Array$<typeof Includable>, {
        nullable: true;
    }>;
    parallel_tool_calls: S.optionalWith<typeof S.Boolean, {
        nullable: true;
        default: () => true;
    }>;
    store: S.optionalWith<typeof S.Boolean, {
        nullable: true;
        default: () => true;
    }>;
    stream: S.optionalWith<typeof S.Boolean, {
        nullable: true;
        default: () => false;
    }>;
    previous_response_id: S.optionalWith<typeof S.String, {
        nullable: true;
    }>;
    model: typeof ModelIdsResponses;
    reasoning: S.optionalWith<typeof Reasoning, {
        nullable: true;
    }>;
    max_output_tokens: S.optionalWith<typeof S.Int, {
        nullable: true;
    }>;
    instructions: S.optionalWith<typeof S.String, {
        nullable: true;
    }>;
    text: S.optionalWith<S.Struct<{
        format: S.optionalWith<typeof TextResponseFormatConfiguration, {
            nullable: true;
        }>;
    }>, {
        nullable: true;
    }>;
    tools: S.optionalWith<S.Array$<typeof Tool>, {
        nullable: true;
    }>;
    tool_choice: S.optionalWith<S.Union<[typeof ToolChoiceOptions, typeof ToolChoiceTypes, typeof ToolChoiceFunction]>, {
        nullable: true;
    }>;
    truncation: S.optionalWith<typeof CreateResponseTruncation, {
        nullable: true;
        default: () => "disabled";
    }>;
    metadata: S.optionalWith<typeof Metadata, {
        nullable: true;
    }>;
    temperature: S.optionalWith<S.filter<S.filter<typeof S.Number>>, {
        nullable: true;
        default: () => 1;
    }>;
    top_p: S.optionalWith<S.filter<S.filter<typeof S.Number>>, {
        nullable: true;
        default: () => 1;
    }>;
    user: S.optionalWith<typeof S.String, {
        nullable: true;
    }>;
}>, never, {
    readonly model: string;
} & {
    readonly text?: {
        readonly format?: {
            readonly type: "text";
        } | {
            readonly type: "json_object";
        } | {
            readonly description?: string | undefined;
            readonly type: "json_schema";
            readonly name?: string | undefined;
            readonly strict: boolean;
            readonly schema: {
                readonly [x: string]: unknown;
            };
        } | undefined;
    } | undefined;
} & {
    readonly instructions?: string | undefined;
} & {
    readonly tools?: readonly ({
        readonly type: "file_search";
        readonly max_num_results?: number | undefined;
        readonly ranking_options?: {
            readonly ranker: "auto" | "default-2024-11-15";
            readonly score_threshold: number;
        } | undefined;
        readonly vector_store_ids: readonly string[];
        readonly filters?: {
            readonly value: string | number | boolean;
            readonly type: "eq" | "ne" | "gt" | "gte" | "lt" | "lte";
            readonly key: string;
        } | {
            readonly type: "and" | "or";
            readonly filters: readonly ({
                readonly [x: string]: unknown;
            } | {
                readonly value: string | number | boolean;
                readonly type: "eq" | "ne" | "gt" | "gte" | "lt" | "lte";
                readonly key: string;
            })[];
        } | undefined;
    } | {
        readonly description?: string | undefined;
        readonly type: "function";
        readonly name: string;
        readonly parameters: {
            readonly [x: string]: unknown;
        };
        readonly strict: boolean;
    } | {
        readonly type: "computer_use_preview";
        readonly display_width: number;
        readonly display_height: number;
        readonly environment: "mac" | "windows" | "ubuntu" | "browser";
    } | {
        readonly type: "web_search_preview" | "web_search_preview_2025_03_11";
        readonly user_location?: {
            readonly type: "approximate";
            readonly country?: string | undefined;
            readonly region?: string | undefined;
            readonly city?: string | undefined;
            readonly timezone?: string | undefined;
        } | undefined;
        readonly search_context_size: "low" | "medium" | "high";
    })[] | undefined;
} & {
    readonly metadata?: {
        readonly [x: string]: unknown;
    } | undefined;
} & {
    readonly temperature?: number;
} & {
    readonly top_p?: number;
} & {
    readonly input: string | readonly ({
        readonly [x: string]: unknown;
    } | {
        readonly content: string | readonly ({
            readonly type: "input_text";
            readonly text: string;
        } | {
            readonly type: "input_image";
            readonly image_url?: string | undefined;
            readonly detail: "auto" | "low" | "high";
            readonly file_id?: string | undefined;
        } | {
            readonly type: "input_file";
            readonly filename?: string | undefined;
            readonly file_data?: string | undefined;
            readonly file_id?: string | undefined;
        })[];
        readonly role: "assistant" | "developer" | "system" | "user";
        readonly type?: "message" | undefined;
    } | {
        readonly type: "item_reference";
        readonly id: string;
    })[];
} & {
    readonly user?: string | undefined;
} & {
    readonly store?: boolean;
} & {
    readonly stream?: boolean;
} & {
    readonly tool_choice?: "none" | "auto" | "required" | {
        readonly type: "file_search" | "computer_use_preview" | "web_search_preview" | "web_search_preview_2025_03_11";
    } | {
        readonly type: "function";
        readonly name: string;
    } | undefined;
} & {
    readonly parallel_tool_calls?: boolean;
} & {
    readonly include?: readonly ("file_search_call.results" | "message.input_image.image_url" | "computer_call_output.output.image_url")[] | undefined;
} & {
    readonly reasoning?: {
        readonly effort: "low" | "medium" | "high";
        readonly generate_summary?: "concise" | "detailed" | undefined;
    } | undefined;
} & {
    readonly previous_response_id?: string | undefined;
} & {
    readonly max_output_tokens?: number | undefined;
} & {
    readonly truncation?: "auto" | "disabled";
}, {}, {}>;
export declare class CreateResponse extends CreateResponse_base {
}
declare const ResponseObject_base: S.Literal<["response"]>;
export declare class ResponseObject extends ResponseObject_base {
}
declare const ResponseStatus_base: S.Literal<["completed", "failed", "in_progress", "incomplete"]>;
export declare class ResponseStatus extends ResponseStatus_base {
}
declare const ResponseErrorCode_base: S.Literal<["server_error", "rate_limit_exceeded", "invalid_prompt", "vector_store_timeout", "invalid_image", "invalid_image_format", "invalid_base64_image", "invalid_image_url", "image_too_large", "image_too_small", "image_parse_error", "image_content_policy_violation", "invalid_image_mode", "image_file_too_large", "unsupported_image_media_type", "empty_image_file", "failed_to_download_image", "image_file_not_found"]>;
export declare class ResponseErrorCode extends ResponseErrorCode_base {
}
declare const ResponseError_base: S.Struct<{
    code: typeof ResponseErrorCode;
    message: typeof S.String;
}>;
export declare class ResponseError extends ResponseError_base {
}
declare const ResponseIncompleteDetailsReason_base: S.Literal<["max_output_tokens", "content_filter"]>;
export declare class ResponseIncompleteDetailsReason extends ResponseIncompleteDetailsReason_base {
}
declare const OutputItem_base: S.Union<[typeof OutputMessage, typeof FileSearchToolCall, typeof FunctionToolCall, typeof WebSearchToolCall, typeof ComputerToolCall, typeof ReasoningItem]>;
export declare class OutputItem extends OutputItem_base {
}
declare const ResponseUsage_base: S.Struct<{
    input_tokens: typeof S.Int;
    input_tokens_details: S.Struct<{
        cached_tokens: typeof S.Int;
    }>;
    output_tokens: typeof S.Int;
    output_tokens_details: S.Struct<{
        reasoning_tokens: typeof S.Int;
    }>;
    total_tokens: typeof S.Int;
}>;
export declare class ResponseUsage extends ResponseUsage_base {
}
declare const ResponseTruncation_base: S.Literal<["auto", "disabled"]>;
export declare class ResponseTruncation extends ResponseTruncation_base {
}
declare const Response_base: S.Class<Response, {
    id: typeof S.String;
    object: typeof ResponseObject;
    status: S.optionalWith<typeof ResponseStatus, {
        nullable: true;
    }>;
    created_at: typeof S.Number;
    error: S.NullOr<typeof ResponseError>;
    incomplete_details: S.NullOr<S.Struct<{
        reason: S.optionalWith<typeof ResponseIncompleteDetailsReason, {
            nullable: true;
        }>;
    }>>;
    output: S.Array$<typeof OutputItem>;
    output_text: S.optionalWith<typeof S.String, {
        nullable: true;
    }>;
    usage: S.optionalWith<typeof ResponseUsage, {
        nullable: true;
    }>;
    parallel_tool_calls: S.PropertySignature<":", boolean, never, ":", boolean, true, never>;
    previous_response_id: S.optionalWith<typeof S.String, {
        nullable: true;
    }>;
    model: typeof ModelIdsResponses;
    reasoning: S.optionalWith<typeof Reasoning, {
        nullable: true;
    }>;
    max_output_tokens: S.optionalWith<typeof S.Int, {
        nullable: true;
    }>;
    instructions: S.NullOr<typeof S.String>;
    text: S.optionalWith<S.Struct<{
        format: S.optionalWith<typeof TextResponseFormatConfiguration, {
            nullable: true;
        }>;
    }>, {
        nullable: true;
    }>;
    tools: S.Array$<typeof Tool>;
    tool_choice: S.Union<[typeof ToolChoiceOptions, typeof ToolChoiceTypes, typeof ToolChoiceFunction]>;
    truncation: S.optionalWith<typeof ResponseTruncation, {
        nullable: true;
        default: () => "disabled";
    }>;
    metadata: S.NullOr<typeof Metadata>;
    temperature: S.PropertySignature<":", number | null, never, ":", number | null, true, never>;
    top_p: S.PropertySignature<":", number | null, never, ":", number | null, true, never>;
    user: S.optionalWith<typeof S.String, {
        nullable: true;
    }>;
}, S.Struct.Encoded<{
    id: typeof S.String;
    object: typeof ResponseObject;
    status: S.optionalWith<typeof ResponseStatus, {
        nullable: true;
    }>;
    created_at: typeof S.Number;
    error: S.NullOr<typeof ResponseError>;
    incomplete_details: S.NullOr<S.Struct<{
        reason: S.optionalWith<typeof ResponseIncompleteDetailsReason, {
            nullable: true;
        }>;
    }>>;
    output: S.Array$<typeof OutputItem>;
    output_text: S.optionalWith<typeof S.String, {
        nullable: true;
    }>;
    usage: S.optionalWith<typeof ResponseUsage, {
        nullable: true;
    }>;
    parallel_tool_calls: S.PropertySignature<":", boolean, never, ":", boolean, true, never>;
    previous_response_id: S.optionalWith<typeof S.String, {
        nullable: true;
    }>;
    model: typeof ModelIdsResponses;
    reasoning: S.optionalWith<typeof Reasoning, {
        nullable: true;
    }>;
    max_output_tokens: S.optionalWith<typeof S.Int, {
        nullable: true;
    }>;
    instructions: S.NullOr<typeof S.String>;
    text: S.optionalWith<S.Struct<{
        format: S.optionalWith<typeof TextResponseFormatConfiguration, {
            nullable: true;
        }>;
    }>, {
        nullable: true;
    }>;
    tools: S.Array$<typeof Tool>;
    tool_choice: S.Union<[typeof ToolChoiceOptions, typeof ToolChoiceTypes, typeof ToolChoiceFunction]>;
    truncation: S.optionalWith<typeof ResponseTruncation, {
        nullable: true;
        default: () => "disabled";
    }>;
    metadata: S.NullOr<typeof Metadata>;
    temperature: S.PropertySignature<":", number | null, never, ":", number | null, true, never>;
    top_p: S.PropertySignature<":", number | null, never, ":", number | null, true, never>;
    user: S.optionalWith<typeof S.String, {
        nullable: true;
    }>;
}>, never, {
    readonly object: "response";
} & {
    readonly model: string;
} & {
    readonly text?: {
        readonly format?: {
            readonly type: "text";
        } | {
            readonly type: "json_object";
        } | {
            readonly description?: string | undefined;
            readonly type: "json_schema";
            readonly name?: string | undefined;
            readonly strict: boolean;
            readonly schema: {
                readonly [x: string]: unknown;
            };
        } | undefined;
    } | undefined;
} & {
    readonly id: string;
} & {
    readonly created_at: number;
} & {
    readonly instructions: string | null;
} & {
    readonly tools: readonly ({
        readonly type: "file_search";
        readonly max_num_results?: number | undefined;
        readonly ranking_options?: {
            readonly ranker: "auto" | "default-2024-11-15";
            readonly score_threshold: number;
        } | undefined;
        readonly vector_store_ids: readonly string[];
        readonly filters?: {
            readonly value: string | number | boolean;
            readonly type: "eq" | "ne" | "gt" | "gte" | "lt" | "lte";
            readonly key: string;
        } | {
            readonly type: "and" | "or";
            readonly filters: readonly ({
                readonly [x: string]: unknown;
            } | {
                readonly value: string | number | boolean;
                readonly type: "eq" | "ne" | "gt" | "gte" | "lt" | "lte";
                readonly key: string;
            })[];
        } | undefined;
    } | {
        readonly description?: string | undefined;
        readonly type: "function";
        readonly name: string;
        readonly parameters: {
            readonly [x: string]: unknown;
        };
        readonly strict: boolean;
    } | {
        readonly type: "computer_use_preview";
        readonly display_width: number;
        readonly display_height: number;
        readonly environment: "mac" | "windows" | "ubuntu" | "browser";
    } | {
        readonly type: "web_search_preview" | "web_search_preview_2025_03_11";
        readonly user_location?: {
            readonly type: "approximate";
            readonly country?: string | undefined;
            readonly region?: string | undefined;
            readonly city?: string | undefined;
            readonly timezone?: string | undefined;
        } | undefined;
        readonly search_context_size: "low" | "medium" | "high";
    })[];
} & {
    readonly metadata: {
        readonly [x: string]: unknown;
    } | null;
} & {
    readonly temperature?: number | null;
} & {
    readonly top_p?: number | null;
} & {
    readonly status?: "failed" | "in_progress" | "completed" | "incomplete" | undefined;
} & {
    readonly usage?: {
        readonly total_tokens: number;
        readonly input_tokens: number;
        readonly output_tokens: number;
        readonly input_tokens_details: {
            readonly cached_tokens: number;
        };
        readonly output_tokens_details: {
            readonly reasoning_tokens: number;
        };
    } | undefined;
} & {
    readonly user?: string | undefined;
} & {
    readonly tool_choice: "none" | "auto" | "required" | {
        readonly type: "file_search" | "computer_use_preview" | "web_search_preview" | "web_search_preview_2025_03_11";
    } | {
        readonly type: "function";
        readonly name: string;
    };
} & {
    readonly parallel_tool_calls?: boolean;
} & {
    readonly error: {
        readonly message: string;
        readonly code: "server_error" | "rate_limit_exceeded" | "invalid_prompt" | "vector_store_timeout" | "invalid_image" | "invalid_image_format" | "invalid_base64_image" | "invalid_image_url" | "image_too_large" | "image_too_small" | "image_parse_error" | "image_content_policy_violation" | "invalid_image_mode" | "image_file_too_large" | "unsupported_image_media_type" | "empty_image_file" | "failed_to_download_image" | "image_file_not_found";
    } | null;
} & {
    readonly output_text?: string | undefined;
} & {
    readonly output: readonly ({
        readonly content: readonly ({
            readonly annotations: readonly ({
                readonly type: "file_citation";
                readonly index: number;
                readonly file_id: string;
            } | {
                readonly type: "url_citation";
                readonly end_index: number;
                readonly start_index: number;
                readonly url: string;
                readonly title: string;
            } | {
                readonly type: "file_path";
                readonly index: number;
                readonly file_id: string;
            })[];
            readonly type: "output_text";
            readonly text: string;
        } | {
            readonly type: "refusal";
            readonly refusal: string;
        })[];
        readonly role: "assistant";
        readonly type: "message";
        readonly id: string;
        readonly status: "in_progress" | "completed" | "incomplete";
    } | {
        readonly type: "file_search_call";
        readonly id: string;
        readonly status: "failed" | "in_progress" | "completed" | "incomplete" | "searching";
        readonly results?: readonly {
            readonly text?: string | undefined;
            readonly filename?: string | undefined;
            readonly file_id?: string | undefined;
            readonly attributes?: {
                readonly [x: string]: unknown;
            } | undefined;
            readonly score?: number | undefined;
        }[] | undefined;
        readonly queries: readonly string[];
    } | {
        readonly type: "computer_call";
        readonly id: string;
        readonly status: "in_progress" | "completed" | "incomplete";
        readonly call_id: string;
        readonly action: {
            readonly type: "click";
            readonly button: "left" | "right" | "wheel" | "back" | "forward";
            readonly x: number;
            readonly y: number;
        } | {
            readonly type: "double_click";
            readonly x: number;
            readonly y: number;
        } | {
            readonly type: "drag";
            readonly path: readonly {
                readonly x: number;
                readonly y: number;
            }[];
        } | {
            readonly keys: readonly string[];
            readonly type: "keypress";
        } | {
            readonly type: "move";
            readonly x: number;
            readonly y: number;
        } | {
            readonly type: "screenshot";
        } | {
            readonly type: "scroll";
            readonly x: number;
            readonly y: number;
            readonly scroll_x: number;
            readonly scroll_y: number;
        } | {
            readonly type: "type";
            readonly text: string;
        } | {
            readonly type: "wait";
        };
        readonly pending_safety_checks: readonly {
            readonly message: string;
            readonly id: string;
            readonly code: string;
        }[];
    } | {
        readonly type: "web_search_call";
        readonly id: string;
        readonly status: "failed" | "in_progress" | "completed" | "searching";
    } | {
        readonly type: "function_call";
        readonly name: string;
        readonly id?: string | undefined;
        readonly status?: "in_progress" | "completed" | "incomplete" | undefined;
        readonly arguments: string;
        readonly call_id: string;
    } | {
        readonly summary: readonly {
            readonly type: "summary_text";
            readonly text: string;
        }[];
        readonly type: "reasoning";
        readonly id: string;
        readonly status?: "in_progress" | "completed" | "incomplete" | undefined;
    })[];
} & {
    readonly reasoning?: {
        readonly effort: "low" | "medium" | "high";
        readonly generate_summary?: "concise" | "detailed" | undefined;
    } | undefined;
} & {
    readonly previous_response_id?: string | undefined;
} & {
    readonly max_output_tokens?: number | undefined;
} & {
    readonly truncation?: "auto" | "disabled";
} & {
    readonly incomplete_details: {
        readonly reason?: "content_filter" | "max_output_tokens" | undefined;
    } | null;
}, {}, {}>;
export declare class Response extends Response_base {
}
declare const GetResponseParams_base: S.Struct<{
    include: S.optionalWith<S.Array$<typeof Includable>, {
        nullable: true;
    }>;
}>;
export declare class GetResponseParams extends GetResponseParams_base {
}
declare const ListInputItemsParamsOrder_base: S.Literal<["asc", "desc"]>;
export declare class ListInputItemsParamsOrder extends ListInputItemsParamsOrder_base {
}
declare const ListInputItemsParams_base: S.Struct<{
    limit: S.optionalWith<typeof S.Int, {
        nullable: true;
        default: () => 20;
    }>;
    order: S.optionalWith<typeof ListInputItemsParamsOrder, {
        nullable: true;
    }>;
    after: S.optionalWith<typeof S.String, {
        nullable: true;
    }>;
    before: S.optionalWith<typeof S.String, {
        nullable: true;
    }>;
}>;
export declare class ListInputItemsParams extends ListInputItemsParams_base {
}
declare const ResponseItemListObject_base: S.Literal<["list"]>;
export declare class ResponseItemListObject extends ResponseItemListObject_base {
}
declare const InputMessageResourceType_base: S.Literal<["message"]>;
export declare class InputMessageResourceType extends InputMessageResourceType_base {
}
declare const InputMessageResourceRole_base: S.Literal<["user", "system", "developer"]>;
export declare class InputMessageResourceRole extends InputMessageResourceRole_base {
}
declare const InputMessageResourceStatus_base: S.Literal<["in_progress", "completed", "incomplete"]>;
export declare class InputMessageResourceStatus extends InputMessageResourceStatus_base {
}
declare const InputMessageResource_base: S.Struct<{
    id: typeof S.String;
    type: S.optionalWith<typeof InputMessageResourceType, {
        nullable: true;
    }>;
    role: typeof InputMessageResourceRole;
    status: S.optionalWith<typeof InputMessageResourceStatus, {
        nullable: true;
    }>;
    content: typeof InputMessageContentList;
}>;
export declare class InputMessageResource extends InputMessageResource_base {
}
declare const ComputerToolCallOutputResourceType_base: S.Literal<["computer_call_output"]>;
export declare class ComputerToolCallOutputResourceType extends ComputerToolCallOutputResourceType_base {
}
declare const ComputerToolCallOutputResourceStatus_base: S.Literal<["in_progress", "completed", "incomplete"]>;
export declare class ComputerToolCallOutputResourceStatus extends ComputerToolCallOutputResourceStatus_base {
}
declare const ComputerToolCallOutputResource_base: S.Struct<{
    id: typeof S.String;
    type: S.PropertySignature<":", "computer_call_output", never, ":", "computer_call_output", true, never>;
    call_id: typeof S.String;
    acknowledged_safety_checks: S.optionalWith<S.Array$<typeof ComputerToolCallSafetyCheck>, {
        nullable: true;
    }>;
    output: typeof ComputerScreenshotImage;
    status: S.optionalWith<typeof ComputerToolCallOutputResourceStatus, {
        nullable: true;
    }>;
}>;
export declare class ComputerToolCallOutputResource extends ComputerToolCallOutputResource_base {
}
declare const FunctionToolCallResourceType_base: S.Literal<["function_call"]>;
export declare class FunctionToolCallResourceType extends FunctionToolCallResourceType_base {
}
declare const FunctionToolCallResourceStatus_base: S.Literal<["in_progress", "completed", "incomplete"]>;
export declare class FunctionToolCallResourceStatus extends FunctionToolCallResourceStatus_base {
}
declare const FunctionToolCallResource_base: S.Struct<{
    id: typeof S.String;
    type: typeof FunctionToolCallResourceType;
    call_id: typeof S.String;
    name: typeof S.String;
    arguments: typeof S.String;
    status: S.optionalWith<typeof FunctionToolCallResourceStatus, {
        nullable: true;
    }>;
}>;
export declare class FunctionToolCallResource extends FunctionToolCallResource_base {
}
declare const FunctionToolCallOutputResourceType_base: S.Literal<["function_call_output"]>;
export declare class FunctionToolCallOutputResourceType extends FunctionToolCallOutputResourceType_base {
}
declare const FunctionToolCallOutputResourceStatus_base: S.Literal<["in_progress", "completed", "incomplete"]>;
export declare class FunctionToolCallOutputResourceStatus extends FunctionToolCallOutputResourceStatus_base {
}
declare const FunctionToolCallOutputResource_base: S.Struct<{
    id: typeof S.String;
    type: typeof FunctionToolCallOutputResourceType;
    call_id: typeof S.String;
    output: typeof S.String;
    status: S.optionalWith<typeof FunctionToolCallOutputResourceStatus, {
        nullable: true;
    }>;
}>;
export declare class FunctionToolCallOutputResource extends FunctionToolCallOutputResource_base {
}
declare const ItemResource_base: S.Union<[typeof InputMessageResource, typeof OutputMessage, typeof FileSearchToolCall, typeof ComputerToolCall, typeof ComputerToolCallOutputResource, typeof WebSearchToolCall, typeof FunctionToolCallResource, typeof FunctionToolCallOutputResource]>;
export declare class ItemResource extends ItemResource_base {
}
declare const ResponseItemList_base: S.Class<ResponseItemList, {
    object: typeof ResponseItemListObject;
    data: S.Array$<typeof ItemResource>;
    has_more: typeof S.Boolean;
    first_id: typeof S.String;
    last_id: typeof S.String;
}, S.Struct.Encoded<{
    object: typeof ResponseItemListObject;
    data: S.Array$<typeof ItemResource>;
    has_more: typeof S.Boolean;
    first_id: typeof S.String;
    last_id: typeof S.String;
}>, never, {
    readonly object: "list";
} & {
    readonly first_id: string;
} & {
    readonly last_id: string;
} & {
    readonly has_more: boolean;
} & {
    readonly data: readonly ({
        readonly content: readonly ({
            readonly annotations: readonly ({
                readonly type: "file_citation";
                readonly index: number;
                readonly file_id: string;
            } | {
                readonly type: "url_citation";
                readonly end_index: number;
                readonly start_index: number;
                readonly url: string;
                readonly title: string;
            } | {
                readonly type: "file_path";
                readonly index: number;
                readonly file_id: string;
            })[];
            readonly type: "output_text";
            readonly text: string;
        } | {
            readonly type: "refusal";
            readonly refusal: string;
        })[];
        readonly role: "assistant";
        readonly type: "message";
        readonly id: string;
        readonly status: "in_progress" | "completed" | "incomplete";
    } | {
        readonly type: "file_search_call";
        readonly id: string;
        readonly status: "failed" | "in_progress" | "completed" | "incomplete" | "searching";
        readonly results?: readonly {
            readonly text?: string | undefined;
            readonly filename?: string | undefined;
            readonly file_id?: string | undefined;
            readonly attributes?: {
                readonly [x: string]: unknown;
            } | undefined;
            readonly score?: number | undefined;
        }[] | undefined;
        readonly queries: readonly string[];
    } | {
        readonly type: "computer_call";
        readonly id: string;
        readonly status: "in_progress" | "completed" | "incomplete";
        readonly call_id: string;
        readonly action: {
            readonly type: "click";
            readonly button: "left" | "right" | "wheel" | "back" | "forward";
            readonly x: number;
            readonly y: number;
        } | {
            readonly type: "double_click";
            readonly x: number;
            readonly y: number;
        } | {
            readonly type: "drag";
            readonly path: readonly {
                readonly x: number;
                readonly y: number;
            }[];
        } | {
            readonly keys: readonly string[];
            readonly type: "keypress";
        } | {
            readonly type: "move";
            readonly x: number;
            readonly y: number;
        } | {
            readonly type: "screenshot";
        } | {
            readonly type: "scroll";
            readonly x: number;
            readonly y: number;
            readonly scroll_x: number;
            readonly scroll_y: number;
        } | {
            readonly type: "type";
            readonly text: string;
        } | {
            readonly type: "wait";
        };
        readonly pending_safety_checks: readonly {
            readonly message: string;
            readonly id: string;
            readonly code: string;
        }[];
    } | {
        readonly type: "web_search_call";
        readonly id: string;
        readonly status: "failed" | "in_progress" | "completed" | "searching";
    } | {
        readonly content: readonly ({
            readonly type: "input_text";
            readonly text: string;
        } | {
            readonly type: "input_image";
            readonly image_url?: string | undefined;
            readonly detail: "auto" | "low" | "high";
            readonly file_id?: string | undefined;
        } | {
            readonly type: "input_file";
            readonly filename?: string | undefined;
            readonly file_data?: string | undefined;
            readonly file_id?: string | undefined;
        })[];
        readonly role: "developer" | "system" | "user";
        readonly type?: "message" | undefined;
        readonly id: string;
        readonly status?: "in_progress" | "completed" | "incomplete" | undefined;
    } | {
        readonly type: "computer_call_output";
        readonly id: string;
        readonly status?: "in_progress" | "completed" | "incomplete" | undefined;
        readonly call_id: string;
        readonly acknowledged_safety_checks?: readonly {
            readonly message: string;
            readonly id: string;
            readonly code: string;
        }[] | undefined;
        readonly output: {
            readonly type: "computer_screenshot";
            readonly image_url?: string | undefined;
            readonly file_id?: string | undefined;
        };
    } | {
        readonly type: "function_call";
        readonly name: string;
        readonly id: string;
        readonly status?: "in_progress" | "completed" | "incomplete" | undefined;
        readonly arguments: string;
        readonly call_id: string;
    } | {
        readonly type: "function_call_output";
        readonly id: string;
        readonly status?: "in_progress" | "completed" | "incomplete" | undefined;
        readonly call_id: string;
        readonly output: string;
    })[];
}, {}, {}>;
export declare class ResponseItemList extends ResponseItemList_base {
}
declare const CreateMessageRequestRole_base: S.Literal<["user", "assistant"]>;
export declare class CreateMessageRequestRole extends CreateMessageRequestRole_base {
}
declare const MessageContentImageFileObjectType_base: S.Literal<["image_file"]>;
export declare class MessageContentImageFileObjectType extends MessageContentImageFileObjectType_base {
}
declare const MessageContentImageFileObjectImageFileDetail_base: S.Literal<["auto", "low", "high"]>;
export declare class MessageContentImageFileObjectImageFileDetail extends MessageContentImageFileObjectImageFileDetail_base {
}
declare const MessageContentImageFileObject_base: S.Struct<{
    type: typeof MessageContentImageFileObjectType;
    image_file: S.Struct<{
        file_id: typeof S.String;
        detail: S.optionalWith<typeof MessageContentImageFileObjectImageFileDetail, {
            nullable: true;
            default: () => "auto";
        }>;
    }>;
}>;
export declare class MessageContentImageFileObject extends MessageContentImageFileObject_base {
}
declare const MessageContentImageUrlObjectType_base: S.Literal<["image_url"]>;
export declare class MessageContentImageUrlObjectType extends MessageContentImageUrlObjectType_base {
}
declare const MessageContentImageUrlObjectImageUrlDetail_base: S.Literal<["auto", "low", "high"]>;
export declare class MessageContentImageUrlObjectImageUrlDetail extends MessageContentImageUrlObjectImageUrlDetail_base {
}
declare const MessageContentImageUrlObject_base: S.Struct<{
    type: typeof MessageContentImageUrlObjectType;
    image_url: S.Struct<{
        url: typeof S.String;
        detail: S.optionalWith<typeof MessageContentImageUrlObjectImageUrlDetail, {
            nullable: true;
            default: () => "auto";
        }>;
    }>;
}>;
export declare class MessageContentImageUrlObject extends MessageContentImageUrlObject_base {
}
declare const MessageRequestContentTextObjectType_base: S.Literal<["text"]>;
export declare class MessageRequestContentTextObjectType extends MessageRequestContentTextObjectType_base {
}
declare const MessageRequestContentTextObject_base: S.Struct<{
    type: typeof MessageRequestContentTextObjectType;
    text: typeof S.String;
}>;
export declare class MessageRequestContentTextObject extends MessageRequestContentTextObject_base {
}
declare const AssistantToolsFileSearchTypeOnlyType_base: S.Literal<["file_search"]>;
export declare class AssistantToolsFileSearchTypeOnlyType extends AssistantToolsFileSearchTypeOnlyType_base {
}
declare const AssistantToolsFileSearchTypeOnly_base: S.Struct<{
    type: typeof AssistantToolsFileSearchTypeOnlyType;
}>;
export declare class AssistantToolsFileSearchTypeOnly extends AssistantToolsFileSearchTypeOnly_base {
}
declare const CreateMessageRequest_base: S.Struct<{
    role: typeof CreateMessageRequestRole;
    content: S.Union<[typeof S.String, S.NonEmptyArray<S.Union<[typeof MessageContentImageFileObject, typeof MessageContentImageUrlObject, typeof MessageRequestContentTextObject]>>]>;
    attachments: S.optionalWith<S.Array$<S.Struct<{
        file_id: S.optionalWith<typeof S.String, {
            nullable: true;
        }>;
        tools: S.optionalWith<S.Array$<S.Union<[typeof AssistantToolsCode, typeof AssistantToolsFileSearchTypeOnly]>>, {
            nullable: true;
        }>;
    }>>, {
        nullable: true;
    }>;
    metadata: S.optionalWith<typeof Metadata, {
        nullable: true;
    }>;
}>;
export declare class CreateMessageRequest extends CreateMessageRequest_base {
}
declare const CreateThreadRequest_base: S.Class<CreateThreadRequest, {
    messages: S.optionalWith<S.Array$<typeof CreateMessageRequest>, {
        nullable: true;
    }>;
    tool_resources: S.optionalWith<S.Struct<{
        code_interpreter: S.optionalWith<S.Struct<{
            file_ids: S.optionalWith<S.filter<S.Array$<typeof S.String>>, {
                nullable: true;
                default: () => readonly [];
            }>;
        }>, {
            nullable: true;
        }>;
        file_search: S.optionalWith<S.Struct<{
            vector_store_ids: S.optionalWith<S.filter<S.Array$<typeof S.String>>, {
                nullable: true;
            }>;
            vector_stores: S.optionalWith<S.filter<S.Array$<S.Struct<{
                file_ids: S.optionalWith<S.filter<S.Array$<typeof S.String>>, {
                    nullable: true;
                }>;
                chunking_strategy: S.optionalWith<S.Record$<typeof S.String, typeof S.Unknown>, {
                    nullable: true;
                }>;
                metadata: S.optionalWith<typeof Metadata, {
                    nullable: true;
                }>;
            }>>>, {
                nullable: true;
            }>;
        }>, {
            nullable: true;
        }>;
    }>, {
        nullable: true;
    }>;
    metadata: S.optionalWith<typeof Metadata, {
        nullable: true;
    }>;
}, S.Struct.Encoded<{
    messages: S.optionalWith<S.Array$<typeof CreateMessageRequest>, {
        nullable: true;
    }>;
    tool_resources: S.optionalWith<S.Struct<{
        code_interpreter: S.optionalWith<S.Struct<{
            file_ids: S.optionalWith<S.filter<S.Array$<typeof S.String>>, {
                nullable: true;
                default: () => readonly [];
            }>;
        }>, {
            nullable: true;
        }>;
        file_search: S.optionalWith<S.Struct<{
            vector_store_ids: S.optionalWith<S.filter<S.Array$<typeof S.String>>, {
                nullable: true;
            }>;
            vector_stores: S.optionalWith<S.filter<S.Array$<S.Struct<{
                file_ids: S.optionalWith<S.filter<S.Array$<typeof S.String>>, {
                    nullable: true;
                }>;
                chunking_strategy: S.optionalWith<S.Record$<typeof S.String, typeof S.Unknown>, {
                    nullable: true;
                }>;
                metadata: S.optionalWith<typeof Metadata, {
                    nullable: true;
                }>;
            }>>>, {
                nullable: true;
            }>;
        }>, {
            nullable: true;
        }>;
    }>, {
        nullable: true;
    }>;
    metadata: S.optionalWith<typeof Metadata, {
        nullable: true;
    }>;
}>, never, {
    readonly messages?: readonly {
        readonly content: string | readonly [{
            readonly type: "image_file";
            readonly image_file: {
                readonly detail: "auto" | "low" | "high";
                readonly file_id: string;
            };
        } | {
            readonly type: "image_url";
            readonly image_url: {
                readonly url: string;
                readonly detail: "auto" | "low" | "high";
            };
        } | {
            readonly type: "text";
            readonly text: string;
        }, ...({
            readonly type: "image_file";
            readonly image_file: {
                readonly detail: "auto" | "low" | "high";
                readonly file_id: string;
            };
        } | {
            readonly type: "image_url";
            readonly image_url: {
                readonly url: string;
                readonly detail: "auto" | "low" | "high";
            };
        } | {
            readonly type: "text";
            readonly text: string;
        })[]];
        readonly role: "assistant" | "user";
        readonly metadata?: {
            readonly [x: string]: unknown;
        } | undefined;
        readonly attachments?: readonly {
            readonly tools?: readonly ({
                readonly type: "code_interpreter";
            } | {
                readonly type: "file_search";
            })[] | undefined;
            readonly file_id?: string | undefined;
        }[] | undefined;
    }[] | undefined;
} & {
    readonly tool_resources?: {
        readonly code_interpreter?: {
            readonly file_ids: readonly string[];
        } | undefined;
        readonly file_search?: {
            readonly vector_store_ids?: readonly string[] | undefined;
            readonly vector_stores?: readonly {
                readonly metadata?: {
                    readonly [x: string]: unknown;
                } | undefined;
                readonly file_ids?: readonly string[] | undefined;
                readonly chunking_strategy?: {
                    readonly [x: string]: unknown;
                } | undefined;
            }[] | undefined;
        } | undefined;
    } | undefined;
} & {
    readonly metadata?: {
        readonly [x: string]: unknown;
    } | undefined;
}, {}, {}>;
export declare class CreateThreadRequest extends CreateThreadRequest_base {
}
declare const ThreadObjectObject_base: S.Literal<["thread"]>;
export declare class ThreadObjectObject extends ThreadObjectObject_base {
}
declare const ThreadObject_base: S.Class<ThreadObject, {
    id: typeof S.String;
    object: typeof ThreadObjectObject;
    created_at: typeof S.Int;
    tool_resources: S.NullOr<S.Struct<{
        code_interpreter: S.optionalWith<S.Struct<{
            file_ids: S.optionalWith<S.filter<S.Array$<typeof S.String>>, {
                nullable: true;
                default: () => readonly [];
            }>;
        }>, {
            nullable: true;
        }>;
        file_search: S.optionalWith<S.Struct<{
            vector_store_ids: S.optionalWith<S.filter<S.Array$<typeof S.String>>, {
                nullable: true;
            }>;
        }>, {
            nullable: true;
        }>;
    }>>;
    metadata: S.NullOr<typeof Metadata>;
}, S.Struct.Encoded<{
    id: typeof S.String;
    object: typeof ThreadObjectObject;
    created_at: typeof S.Int;
    tool_resources: S.NullOr<S.Struct<{
        code_interpreter: S.optionalWith<S.Struct<{
            file_ids: S.optionalWith<S.filter<S.Array$<typeof S.String>>, {
                nullable: true;
                default: () => readonly [];
            }>;
        }>, {
            nullable: true;
        }>;
        file_search: S.optionalWith<S.Struct<{
            vector_store_ids: S.optionalWith<S.filter<S.Array$<typeof S.String>>, {
                nullable: true;
            }>;
        }>, {
            nullable: true;
        }>;
    }>>;
    metadata: S.NullOr<typeof Metadata>;
}>, never, {
    readonly object: "thread";
} & {
    readonly id: string;
} & {
    readonly created_at: number;
} & {
    readonly tool_resources: {
        readonly code_interpreter?: {
            readonly file_ids: readonly string[];
        } | undefined;
        readonly file_search?: {
            readonly vector_store_ids?: readonly string[] | undefined;
        } | undefined;
    } | null;
} & {
    readonly metadata: {
        readonly [x: string]: unknown;
    } | null;
}, {}, {}>;
export declare class ThreadObject extends ThreadObject_base {
}
declare const CreateThreadAndRunRequestModelEnum_base: S.Literal<["gpt-4o", "gpt-4o-2024-11-20", "gpt-4o-2024-08-06", "gpt-4o-2024-05-13", "gpt-4o-mini", "gpt-4o-mini-2024-07-18", "gpt-4.5-preview", "gpt-4.5-preview-2025-02-27", "gpt-4-turbo", "gpt-4-turbo-2024-04-09", "gpt-4-0125-preview", "gpt-4-turbo-preview", "gpt-4-1106-preview", "gpt-4-vision-preview", "gpt-4", "gpt-4-0314", "gpt-4-0613", "gpt-4-32k", "gpt-4-32k-0314", "gpt-4-32k-0613", "gpt-3.5-turbo", "gpt-3.5-turbo-16k", "gpt-3.5-turbo-0613", "gpt-3.5-turbo-1106", "gpt-3.5-turbo-0125", "gpt-3.5-turbo-16k-0613"]>;
export declare class CreateThreadAndRunRequestModelEnum extends CreateThreadAndRunRequestModelEnum_base {
}
declare const CreateThreadAndRunRequestTruncationStrategyEnumType_base: S.Literal<["auto", "last_messages"]>;
export declare class CreateThreadAndRunRequestTruncationStrategyEnumType extends CreateThreadAndRunRequestTruncationStrategyEnumType_base {
}
declare const CreateThreadAndRunRequestTruncationStrategy_base: S.Struct<{
    type: S.Literal<["auto", "last_messages"]>;
    last_messages: S.optionalWith<S.filter<typeof S.Int>, {
        nullable: true;
    }>;
}>;
export declare class CreateThreadAndRunRequestTruncationStrategy extends CreateThreadAndRunRequestTruncationStrategy_base {
}
declare const CreateThreadAndRunRequestToolChoiceEnumEnum_base: S.Literal<["none", "auto", "required"]>;
export declare class CreateThreadAndRunRequestToolChoiceEnumEnum extends CreateThreadAndRunRequestToolChoiceEnumEnum_base {
}
declare const AssistantsNamedToolChoiceType_base: S.Literal<["function", "code_interpreter", "file_search"]>;
export declare class AssistantsNamedToolChoiceType extends AssistantsNamedToolChoiceType_base {
}
declare const AssistantsNamedToolChoice_base: S.Struct<{
    type: typeof AssistantsNamedToolChoiceType;
    function: S.optionalWith<S.Struct<{
        name: typeof S.String;
    }>, {
        nullable: true;
    }>;
}>;
export declare class AssistantsNamedToolChoice extends AssistantsNamedToolChoice_base {
}
declare const CreateThreadAndRunRequestToolChoice_base: S.Union<[S.Literal<["none", "auto", "required"]>, typeof AssistantsNamedToolChoice]>;
export declare class CreateThreadAndRunRequestToolChoice extends CreateThreadAndRunRequestToolChoice_base {
}
declare const CreateThreadAndRunRequest_base: S.Class<CreateThreadAndRunRequest, {
    assistant_id: typeof S.String;
    thread: S.optionalWith<typeof CreateThreadRequest, {
        nullable: true;
    }>;
    model: S.optionalWith<S.Union<[typeof S.String, typeof CreateThreadAndRunRequestModelEnum]>, {
        nullable: true;
    }>;
    instructions: S.optionalWith<typeof S.String, {
        nullable: true;
    }>;
    tools: S.optionalWith<S.filter<S.Array$<S.Union<[typeof AssistantToolsCode, typeof AssistantToolsFileSearch, typeof AssistantToolsFunction]>>>, {
        nullable: true;
    }>;
    tool_resources: S.optionalWith<S.Struct<{
        code_interpreter: S.optionalWith<S.Struct<{
            file_ids: S.optionalWith<S.filter<S.Array$<typeof S.String>>, {
                nullable: true;
                default: () => readonly [];
            }>;
        }>, {
            nullable: true;
        }>;
        file_search: S.optionalWith<S.Struct<{
            vector_store_ids: S.optionalWith<S.filter<S.Array$<typeof S.String>>, {
                nullable: true;
            }>;
        }>, {
            nullable: true;
        }>;
    }>, {
        nullable: true;
    }>;
    metadata: S.optionalWith<typeof Metadata, {
        nullable: true;
    }>;
    temperature: S.optionalWith<S.filter<S.filter<typeof S.Number>>, {
        nullable: true;
        default: () => 1;
    }>;
    top_p: S.optionalWith<S.filter<S.filter<typeof S.Number>>, {
        nullable: true;
        default: () => 1;
    }>;
    stream: S.optionalWith<typeof S.Boolean, {
        nullable: true;
    }>;
    max_prompt_tokens: S.optionalWith<S.filter<typeof S.Int>, {
        nullable: true;
    }>;
    max_completion_tokens: S.optionalWith<S.filter<typeof S.Int>, {
        nullable: true;
    }>;
    truncation_strategy: S.optionalWith<typeof CreateThreadAndRunRequestTruncationStrategy, {
        nullable: true;
    }>;
    tool_choice: S.optionalWith<typeof CreateThreadAndRunRequestToolChoice, {
        nullable: true;
    }>;
    parallel_tool_calls: S.optionalWith<typeof ParallelToolCalls, {
        nullable: true;
        default: () => true;
    }>;
    response_format: S.optionalWith<typeof AssistantsApiResponseFormatOption, {
        nullable: true;
    }>;
}, S.Struct.Encoded<{
    assistant_id: typeof S.String;
    thread: S.optionalWith<typeof CreateThreadRequest, {
        nullable: true;
    }>;
    model: S.optionalWith<S.Union<[typeof S.String, typeof CreateThreadAndRunRequestModelEnum]>, {
        nullable: true;
    }>;
    instructions: S.optionalWith<typeof S.String, {
        nullable: true;
    }>;
    tools: S.optionalWith<S.filter<S.Array$<S.Union<[typeof AssistantToolsCode, typeof AssistantToolsFileSearch, typeof AssistantToolsFunction]>>>, {
        nullable: true;
    }>;
    tool_resources: S.optionalWith<S.Struct<{
        code_interpreter: S.optionalWith<S.Struct<{
            file_ids: S.optionalWith<S.filter<S.Array$<typeof S.String>>, {
                nullable: true;
                default: () => readonly [];
            }>;
        }>, {
            nullable: true;
        }>;
        file_search: S.optionalWith<S.Struct<{
            vector_store_ids: S.optionalWith<S.filter<S.Array$<typeof S.String>>, {
                nullable: true;
            }>;
        }>, {
            nullable: true;
        }>;
    }>, {
        nullable: true;
    }>;
    metadata: S.optionalWith<typeof Metadata, {
        nullable: true;
    }>;
    temperature: S.optionalWith<S.filter<S.filter<typeof S.Number>>, {
        nullable: true;
        default: () => 1;
    }>;
    top_p: S.optionalWith<S.filter<S.filter<typeof S.Number>>, {
        nullable: true;
        default: () => 1;
    }>;
    stream: S.optionalWith<typeof S.Boolean, {
        nullable: true;
    }>;
    max_prompt_tokens: S.optionalWith<S.filter<typeof S.Int>, {
        nullable: true;
    }>;
    max_completion_tokens: S.optionalWith<S.filter<typeof S.Int>, {
        nullable: true;
    }>;
    truncation_strategy: S.optionalWith<typeof CreateThreadAndRunRequestTruncationStrategy, {
        nullable: true;
    }>;
    tool_choice: S.optionalWith<typeof CreateThreadAndRunRequestToolChoice, {
        nullable: true;
    }>;
    parallel_tool_calls: S.optionalWith<typeof ParallelToolCalls, {
        nullable: true;
        default: () => true;
    }>;
    response_format: S.optionalWith<typeof AssistantsApiResponseFormatOption, {
        nullable: true;
    }>;
}>, never, {
    readonly model?: string | undefined;
} & {
    readonly instructions?: string | undefined;
} & {
    readonly tools?: readonly ({
        readonly type: "code_interpreter";
    } | {
        readonly type: "file_search";
        readonly file_search?: {
            readonly max_num_results?: number | undefined;
            readonly ranking_options?: {
                readonly ranker?: "auto" | "default_2024_08_21" | undefined;
                readonly score_threshold: number;
            } | undefined;
        } | undefined;
    } | {
        readonly function: {
            readonly description?: string | undefined;
            readonly name: string;
            readonly parameters?: {
                readonly [x: string]: unknown;
            } | undefined;
            readonly strict: boolean;
        };
        readonly type: "function";
    })[] | undefined;
} & {
    readonly tool_resources?: {
        readonly code_interpreter?: {
            readonly file_ids: readonly string[];
        } | undefined;
        readonly file_search?: {
            readonly vector_store_ids?: readonly string[] | undefined;
        } | undefined;
    } | undefined;
} & {
    readonly metadata?: {
        readonly [x: string]: unknown;
    } | undefined;
} & {
    readonly temperature?: number;
} & {
    readonly top_p?: number;
} & {
    readonly response_format?: "auto" | {
        readonly type: "text";
    } | {
        readonly type: "json_object";
    } | {
        readonly type: "json_schema";
        readonly json_schema: {
            readonly description?: string | undefined;
            readonly name: string;
            readonly strict: boolean;
            readonly schema?: {
                readonly [x: string]: unknown;
            } | undefined;
        };
    } | undefined;
} & {
    readonly max_completion_tokens?: number | undefined;
} & {
    readonly stream?: boolean | undefined;
} & {
    readonly tool_choice?: "none" | "auto" | "required" | {
        readonly function?: {
            readonly name: string;
        } | undefined;
        readonly type: "function" | "code_interpreter" | "file_search";
    } | undefined;
} & {
    readonly parallel_tool_calls?: boolean;
} & {
    readonly thread?: CreateThreadRequest | undefined;
} & {
    readonly assistant_id: string;
} & {
    readonly max_prompt_tokens?: number | undefined;
} & {
    readonly truncation_strategy?: {
        readonly type: "auto" | "last_messages";
        readonly last_messages?: number | undefined;
    } | undefined;
}, {}, {}>;
export declare class CreateThreadAndRunRequest extends CreateThreadAndRunRequest_base {
}
declare const RunObjectObject_base: S.Literal<["thread.run"]>;
export declare class RunObjectObject extends RunObjectObject_base {
}
declare const RunObjectStatus_base: S.Literal<["queued", "in_progress", "requires_action", "cancelling", "cancelled", "failed", "completed", "incomplete", "expired"]>;
export declare class RunObjectStatus extends RunObjectStatus_base {
}
declare const RunObjectRequiredActionType_base: S.Literal<["submit_tool_outputs"]>;
export declare class RunObjectRequiredActionType extends RunObjectRequiredActionType_base {
}
declare const RunToolCallObjectType_base: S.Literal<["function"]>;
export declare class RunToolCallObjectType extends RunToolCallObjectType_base {
}
declare const RunToolCallObject_base: S.Struct<{
    id: typeof S.String;
    type: typeof RunToolCallObjectType;
    function: S.Struct<{
        name: typeof S.String;
        arguments: typeof S.String;
    }>;
}>;
export declare class RunToolCallObject extends RunToolCallObject_base {
}
declare const RunObjectLastErrorCode_base: S.Literal<["server_error", "rate_limit_exceeded", "invalid_prompt"]>;
export declare class RunObjectLastErrorCode extends RunObjectLastErrorCode_base {
}
declare const RunObjectIncompleteDetailsReason_base: S.Literal<["max_completion_tokens", "max_prompt_tokens"]>;
export declare class RunObjectIncompleteDetailsReason extends RunObjectIncompleteDetailsReason_base {
}
declare const RunCompletionUsage_base: S.Struct<{
    completion_tokens: typeof S.Int;
    prompt_tokens: typeof S.Int;
    total_tokens: typeof S.Int;
}>;
export declare class RunCompletionUsage extends RunCompletionUsage_base {
}
declare const RunObjectTruncationStrategyEnumType_base: S.Literal<["auto", "last_messages"]>;
export declare class RunObjectTruncationStrategyEnumType extends RunObjectTruncationStrategyEnumType_base {
}
declare const RunObjectTruncationStrategy_base: S.Struct<{
    type: S.Literal<["auto", "last_messages"]>;
    last_messages: S.optionalWith<S.filter<typeof S.Int>, {
        nullable: true;
    }>;
}>;
export declare class RunObjectTruncationStrategy extends RunObjectTruncationStrategy_base {
}
declare const RunObjectToolChoiceEnumEnum_base: S.Literal<["none", "auto", "required"]>;
export declare class RunObjectToolChoiceEnumEnum extends RunObjectToolChoiceEnumEnum_base {
}
declare const RunObjectToolChoice_base: S.Union<[S.Literal<["none", "auto", "required"]>, typeof AssistantsNamedToolChoice]>;
export declare class RunObjectToolChoice extends RunObjectToolChoice_base {
}
declare const RunObject_base: S.Class<RunObject, {
    id: typeof S.String;
    object: typeof RunObjectObject;
    created_at: typeof S.Int;
    thread_id: typeof S.String;
    assistant_id: typeof S.String;
    status: typeof RunObjectStatus;
    required_action: S.NullOr<S.Struct<{
        type: typeof RunObjectRequiredActionType;
        submit_tool_outputs: S.Struct<{
            tool_calls: S.Array$<typeof RunToolCallObject>;
        }>;
    }>>;
    last_error: S.NullOr<S.Struct<{
        code: typeof RunObjectLastErrorCode;
        message: typeof S.String;
    }>>;
    expires_at: S.NullOr<typeof S.Int>;
    started_at: S.NullOr<typeof S.Int>;
    cancelled_at: S.NullOr<typeof S.Int>;
    failed_at: S.NullOr<typeof S.Int>;
    completed_at: S.NullOr<typeof S.Int>;
    incomplete_details: S.NullOr<S.Struct<{
        reason: S.optionalWith<typeof RunObjectIncompleteDetailsReason, {
            nullable: true;
        }>;
    }>>;
    model: typeof S.String;
    instructions: typeof S.String;
    tools: S.PropertySignature<":", readonly ({
        readonly type: "code_interpreter";
    } | {
        readonly type: "file_search";
        readonly file_search?: {
            readonly max_num_results?: number | undefined;
            readonly ranking_options?: {
                readonly ranker?: "auto" | "default_2024_08_21" | undefined;
                readonly score_threshold: number;
            } | undefined;
        } | undefined;
    } | {
        readonly function: {
            readonly description?: string | undefined;
            readonly name: string;
            readonly parameters?: {
                readonly [x: string]: unknown;
            } | undefined;
            readonly strict: boolean;
        };
        readonly type: "function";
    })[], never, ":", readonly ({
        readonly type: "code_interpreter";
    } | {
        readonly type: "file_search";
        readonly file_search?: {
            readonly max_num_results?: number | null | undefined;
            readonly ranking_options?: {
                readonly score_threshold: number;
                readonly ranker?: "auto" | "default_2024_08_21" | null | undefined;
            } | null | undefined;
        } | null | undefined;
    } | {
        readonly function: {
            readonly name: string;
            readonly description?: string | null | undefined;
            readonly parameters?: {
                readonly [x: string]: unknown;
            } | null | undefined;
            readonly strict?: boolean | null | undefined;
        };
        readonly type: "function";
    })[], true, never>;
    metadata: S.NullOr<typeof Metadata>;
    usage: S.NullOr<typeof RunCompletionUsage>;
    temperature: S.optionalWith<typeof S.Number, {
        nullable: true;
    }>;
    top_p: S.optionalWith<typeof S.Number, {
        nullable: true;
    }>;
    max_prompt_tokens: S.NullOr<S.filter<typeof S.Int>>;
    max_completion_tokens: S.NullOr<S.filter<typeof S.Int>>;
    truncation_strategy: typeof RunObjectTruncationStrategy;
    tool_choice: typeof RunObjectToolChoice;
    parallel_tool_calls: S.PropertySignature<":", boolean, never, ":", boolean, true, never>;
    response_format: typeof AssistantsApiResponseFormatOption;
}, S.Struct.Encoded<{
    id: typeof S.String;
    object: typeof RunObjectObject;
    created_at: typeof S.Int;
    thread_id: typeof S.String;
    assistant_id: typeof S.String;
    status: typeof RunObjectStatus;
    required_action: S.NullOr<S.Struct<{
        type: typeof RunObjectRequiredActionType;
        submit_tool_outputs: S.Struct<{
            tool_calls: S.Array$<typeof RunToolCallObject>;
        }>;
    }>>;
    last_error: S.NullOr<S.Struct<{
        code: typeof RunObjectLastErrorCode;
        message: typeof S.String;
    }>>;
    expires_at: S.NullOr<typeof S.Int>;
    started_at: S.NullOr<typeof S.Int>;
    cancelled_at: S.NullOr<typeof S.Int>;
    failed_at: S.NullOr<typeof S.Int>;
    completed_at: S.NullOr<typeof S.Int>;
    incomplete_details: S.NullOr<S.Struct<{
        reason: S.optionalWith<typeof RunObjectIncompleteDetailsReason, {
            nullable: true;
        }>;
    }>>;
    model: typeof S.String;
    instructions: typeof S.String;
    tools: S.PropertySignature<":", readonly ({
        readonly type: "code_interpreter";
    } | {
        readonly type: "file_search";
        readonly file_search?: {
            readonly max_num_results?: number | undefined;
            readonly ranking_options?: {
                readonly ranker?: "auto" | "default_2024_08_21" | undefined;
                readonly score_threshold: number;
            } | undefined;
        } | undefined;
    } | {
        readonly function: {
            readonly description?: string | undefined;
            readonly name: string;
            readonly parameters?: {
                readonly [x: string]: unknown;
            } | undefined;
            readonly strict: boolean;
        };
        readonly type: "function";
    })[], never, ":", readonly ({
        readonly type: "code_interpreter";
    } | {
        readonly type: "file_search";
        readonly file_search?: {
            readonly max_num_results?: number | null | undefined;
            readonly ranking_options?: {
                readonly score_threshold: number;
                readonly ranker?: "auto" | "default_2024_08_21" | null | undefined;
            } | null | undefined;
        } | null | undefined;
    } | {
        readonly function: {
            readonly name: string;
            readonly description?: string | null | undefined;
            readonly parameters?: {
                readonly [x: string]: unknown;
            } | null | undefined;
            readonly strict?: boolean | null | undefined;
        };
        readonly type: "function";
    })[], true, never>;
    metadata: S.NullOr<typeof Metadata>;
    usage: S.NullOr<typeof RunCompletionUsage>;
    temperature: S.optionalWith<typeof S.Number, {
        nullable: true;
    }>;
    top_p: S.optionalWith<typeof S.Number, {
        nullable: true;
    }>;
    max_prompt_tokens: S.NullOr<S.filter<typeof S.Int>>;
    max_completion_tokens: S.NullOr<S.filter<typeof S.Int>>;
    truncation_strategy: typeof RunObjectTruncationStrategy;
    tool_choice: typeof RunObjectToolChoice;
    parallel_tool_calls: S.PropertySignature<":", boolean, never, ":", boolean, true, never>;
    response_format: typeof AssistantsApiResponseFormatOption;
}>, never, {
    readonly object: "thread.run";
} & {
    readonly model: string;
} & {
    readonly id: string;
} & {
    readonly created_at: number;
} & {
    readonly instructions: string;
} & {
    readonly tools?: readonly ({
        readonly type: "code_interpreter";
    } | {
        readonly type: "file_search";
        readonly file_search?: {
            readonly max_num_results?: number | undefined;
            readonly ranking_options?: {
                readonly ranker?: "auto" | "default_2024_08_21" | undefined;
                readonly score_threshold: number;
            } | undefined;
        } | undefined;
    } | {
        readonly function: {
            readonly description?: string | undefined;
            readonly name: string;
            readonly parameters?: {
                readonly [x: string]: unknown;
            } | undefined;
            readonly strict: boolean;
        };
        readonly type: "function";
    })[];
} & {
    readonly metadata: {
        readonly [x: string]: unknown;
    } | null;
} & {
    readonly temperature?: number | undefined;
} & {
    readonly top_p?: number | undefined;
} & {
    readonly response_format: "auto" | {
        readonly type: "text";
    } | {
        readonly type: "json_object";
    } | {
        readonly type: "json_schema";
        readonly json_schema: {
            readonly description?: string | undefined;
            readonly name: string;
            readonly strict: boolean;
            readonly schema?: {
                readonly [x: string]: unknown;
            } | undefined;
        };
    };
} & {
    readonly status: "failed" | "in_progress" | "completed" | "expired" | "cancelling" | "cancelled" | "queued" | "incomplete" | "requires_action";
} & {
    readonly expires_at: number | null;
} & {
    readonly completed_at: number | null;
} & {
    readonly failed_at: number | null;
} & {
    readonly cancelled_at: number | null;
} & {
    readonly usage: {
        readonly completion_tokens: number;
        readonly prompt_tokens: number;
        readonly total_tokens: number;
    } | null;
} & {
    readonly max_completion_tokens: number | null;
} & {
    readonly tool_choice: "none" | "auto" | "required" | {
        readonly function?: {
            readonly name: string;
        } | undefined;
        readonly type: "function" | "code_interpreter" | "file_search";
    };
} & {
    readonly parallel_tool_calls?: boolean;
} & {
    readonly incomplete_details: {
        readonly reason?: "max_completion_tokens" | "max_prompt_tokens" | undefined;
    } | null;
} & {
    readonly assistant_id: string;
} & {
    readonly max_prompt_tokens: number | null;
} & {
    readonly truncation_strategy: {
        readonly type: "auto" | "last_messages";
        readonly last_messages?: number | undefined;
    };
} & {
    readonly thread_id: string;
} & {
    readonly required_action: {
        readonly type: "submit_tool_outputs";
        readonly submit_tool_outputs: {
            readonly tool_calls: readonly {
                readonly function: {
                    readonly name: string;
                    readonly arguments: string;
                };
                readonly type: "function";
                readonly id: string;
            }[];
        };
    } | null;
} & {
    readonly last_error: {
        readonly message: string;
        readonly code: "server_error" | "rate_limit_exceeded" | "invalid_prompt";
    } | null;
} & {
    readonly started_at: number | null;
}, {}, {}>;
export declare class RunObject extends RunObject_base {
}
declare const ModifyThreadRequest_base: S.Class<ModifyThreadRequest, {
    tool_resources: S.optionalWith<S.Struct<{
        code_interpreter: S.optionalWith<S.Struct<{
            file_ids: S.optionalWith<S.filter<S.Array$<typeof S.String>>, {
                nullable: true;
                default: () => readonly [];
            }>;
        }>, {
            nullable: true;
        }>;
        file_search: S.optionalWith<S.Struct<{
            vector_store_ids: S.optionalWith<S.filter<S.Array$<typeof S.String>>, {
                nullable: true;
            }>;
        }>, {
            nullable: true;
        }>;
    }>, {
        nullable: true;
    }>;
    metadata: S.optionalWith<typeof Metadata, {
        nullable: true;
    }>;
}, S.Struct.Encoded<{
    tool_resources: S.optionalWith<S.Struct<{
        code_interpreter: S.optionalWith<S.Struct<{
            file_ids: S.optionalWith<S.filter<S.Array$<typeof S.String>>, {
                nullable: true;
                default: () => readonly [];
            }>;
        }>, {
            nullable: true;
        }>;
        file_search: S.optionalWith<S.Struct<{
            vector_store_ids: S.optionalWith<S.filter<S.Array$<typeof S.String>>, {
                nullable: true;
            }>;
        }>, {
            nullable: true;
        }>;
    }>, {
        nullable: true;
    }>;
    metadata: S.optionalWith<typeof Metadata, {
        nullable: true;
    }>;
}>, never, {
    readonly tool_resources?: {
        readonly code_interpreter?: {
            readonly file_ids: readonly string[];
        } | undefined;
        readonly file_search?: {
            readonly vector_store_ids?: readonly string[] | undefined;
        } | undefined;
    } | undefined;
} & {
    readonly metadata?: {
        readonly [x: string]: unknown;
    } | undefined;
}, {}, {}>;
export declare class ModifyThreadRequest extends ModifyThreadRequest_base {
}
declare const DeleteThreadResponseObject_base: S.Literal<["thread.deleted"]>;
export declare class DeleteThreadResponseObject extends DeleteThreadResponseObject_base {
}
declare const DeleteThreadResponse_base: S.Class<DeleteThreadResponse, {
    id: typeof S.String;
    deleted: typeof S.Boolean;
    object: typeof DeleteThreadResponseObject;
}, S.Struct.Encoded<{
    id: typeof S.String;
    deleted: typeof S.Boolean;
    object: typeof DeleteThreadResponseObject;
}>, never, {
    readonly object: "thread.deleted";
} & {
    readonly id: string;
} & {
    readonly deleted: boolean;
}, {}, {}>;
export declare class DeleteThreadResponse extends DeleteThreadResponse_base {
}
declare const ListMessagesParamsOrder_base: S.Literal<["asc", "desc"]>;
export declare class ListMessagesParamsOrder extends ListMessagesParamsOrder_base {
}
declare const ListMessagesParams_base: S.Struct<{
    limit: S.optionalWith<typeof S.Int, {
        nullable: true;
        default: () => 20;
    }>;
    order: S.optionalWith<typeof ListMessagesParamsOrder, {
        nullable: true;
        default: () => "desc";
    }>;
    after: S.optionalWith<typeof S.String, {
        nullable: true;
    }>;
    before: S.optionalWith<typeof S.String, {
        nullable: true;
    }>;
    run_id: S.optionalWith<typeof S.String, {
        nullable: true;
    }>;
}>;
export declare class ListMessagesParams extends ListMessagesParams_base {
}
declare const MessageObjectObject_base: S.Literal<["thread.message"]>;
export declare class MessageObjectObject extends MessageObjectObject_base {
}
declare const MessageObjectStatus_base: S.Literal<["in_progress", "incomplete", "completed"]>;
export declare class MessageObjectStatus extends MessageObjectStatus_base {
}
declare const MessageObjectIncompleteDetailsReason_base: S.Literal<["content_filter", "max_tokens", "run_cancelled", "run_expired", "run_failed"]>;
export declare class MessageObjectIncompleteDetailsReason extends MessageObjectIncompleteDetailsReason_base {
}
declare const MessageObjectRole_base: S.Literal<["user", "assistant"]>;
export declare class MessageObjectRole extends MessageObjectRole_base {
}
declare const MessageContentTextObjectType_base: S.Literal<["text"]>;
export declare class MessageContentTextObjectType extends MessageContentTextObjectType_base {
}
declare const MessageContentTextAnnotationsFileCitationObjectType_base: S.Literal<["file_citation"]>;
export declare class MessageContentTextAnnotationsFileCitationObjectType extends MessageContentTextAnnotationsFileCitationObjectType_base {
}
declare const MessageContentTextAnnotationsFileCitationObject_base: S.Struct<{
    type: typeof MessageContentTextAnnotationsFileCitationObjectType;
    text: typeof S.String;
    file_citation: S.Struct<{
        file_id: typeof S.String;
    }>;
    start_index: S.filter<typeof S.Int>;
    end_index: S.filter<typeof S.Int>;
}>;
export declare class MessageContentTextAnnotationsFileCitationObject extends MessageContentTextAnnotationsFileCitationObject_base {
}
declare const MessageContentTextAnnotationsFilePathObjectType_base: S.Literal<["file_path"]>;
export declare class MessageContentTextAnnotationsFilePathObjectType extends MessageContentTextAnnotationsFilePathObjectType_base {
}
declare const MessageContentTextAnnotationsFilePathObject_base: S.Struct<{
    type: typeof MessageContentTextAnnotationsFilePathObjectType;
    text: typeof S.String;
    file_path: S.Struct<{
        file_id: typeof S.String;
    }>;
    start_index: S.filter<typeof S.Int>;
    end_index: S.filter<typeof S.Int>;
}>;
export declare class MessageContentTextAnnotationsFilePathObject extends MessageContentTextAnnotationsFilePathObject_base {
}
declare const MessageContentTextObject_base: S.Struct<{
    type: typeof MessageContentTextObjectType;
    text: S.Struct<{
        value: typeof S.String;
        annotations: S.Array$<S.Union<[typeof MessageContentTextAnnotationsFileCitationObject, typeof MessageContentTextAnnotationsFilePathObject]>>;
    }>;
}>;
export declare class MessageContentTextObject extends MessageContentTextObject_base {
}
declare const MessageContentRefusalObjectType_base: S.Literal<["refusal"]>;
export declare class MessageContentRefusalObjectType extends MessageContentRefusalObjectType_base {
}
declare const MessageContentRefusalObject_base: S.Struct<{
    type: typeof MessageContentRefusalObjectType;
    refusal: typeof S.String;
}>;
export declare class MessageContentRefusalObject extends MessageContentRefusalObject_base {
}
declare const MessageObject_base: S.Struct<{
    id: typeof S.String;
    object: typeof MessageObjectObject;
    created_at: typeof S.Int;
    thread_id: typeof S.String;
    status: typeof MessageObjectStatus;
    incomplete_details: S.NullOr<S.Struct<{
        reason: typeof MessageObjectIncompleteDetailsReason;
    }>>;
    completed_at: S.NullOr<typeof S.Int>;
    incomplete_at: S.NullOr<typeof S.Int>;
    role: typeof MessageObjectRole;
    content: S.Array$<S.Union<[typeof MessageContentImageFileObject, typeof MessageContentImageUrlObject, typeof MessageContentTextObject, typeof MessageContentRefusalObject]>>;
    assistant_id: S.NullOr<typeof S.String>;
    run_id: S.NullOr<typeof S.String>;
    attachments: S.NullOr<S.Array$<S.Struct<{
        file_id: S.optionalWith<typeof S.String, {
            nullable: true;
        }>;
        tools: S.optionalWith<S.Array$<S.Union<[typeof AssistantToolsCode, typeof AssistantToolsFileSearchTypeOnly]>>, {
            nullable: true;
        }>;
    }>>>;
    metadata: S.NullOr<typeof Metadata>;
}>;
export declare class MessageObject extends MessageObject_base {
}
declare const ListMessagesResponse_base: S.Class<ListMessagesResponse, {
    object: typeof S.String;
    data: S.Array$<typeof MessageObject>;
    first_id: typeof S.String;
    last_id: typeof S.String;
    has_more: typeof S.Boolean;
}, S.Struct.Encoded<{
    object: typeof S.String;
    data: S.Array$<typeof MessageObject>;
    first_id: typeof S.String;
    last_id: typeof S.String;
    has_more: typeof S.Boolean;
}>, never, {
    readonly object: string;
} & {
    readonly first_id: string;
} & {
    readonly last_id: string;
} & {
    readonly has_more: boolean;
} & {
    readonly data: readonly {
        readonly object: "thread.message";
        readonly content: readonly ({
            readonly type: "image_file";
            readonly image_file: {
                readonly detail: "auto" | "low" | "high";
                readonly file_id: string;
            };
        } | {
            readonly type: "image_url";
            readonly image_url: {
                readonly url: string;
                readonly detail: "auto" | "low" | "high";
            };
        } | {
            readonly type: "text";
            readonly text: {
                readonly annotations: readonly ({
                    readonly type: "file_citation";
                    readonly text: string;
                    readonly end_index: number;
                    readonly start_index: number;
                    readonly file_citation: {
                        readonly file_id: string;
                    };
                } | {
                    readonly type: "file_path";
                    readonly text: string;
                    readonly end_index: number;
                    readonly start_index: number;
                    readonly file_path: {
                        readonly file_id: string;
                    };
                })[];
                readonly value: string;
            };
        } | {
            readonly type: "refusal";
            readonly refusal: string;
        })[];
        readonly role: "assistant" | "user";
        readonly id: string;
        readonly created_at: number;
        readonly metadata: {
            readonly [x: string]: unknown;
        } | null;
        readonly status: "in_progress" | "completed" | "incomplete";
        readonly completed_at: number | null;
        readonly incomplete_details: {
            readonly reason: "content_filter" | "max_tokens" | "run_cancelled" | "run_expired" | "run_failed";
        } | null;
        readonly attachments: readonly {
            readonly tools?: readonly ({
                readonly type: "code_interpreter";
            } | {
                readonly type: "file_search";
            })[] | undefined;
            readonly file_id?: string | undefined;
        }[] | null;
        readonly assistant_id: string | null;
        readonly thread_id: string;
        readonly run_id: string | null;
        readonly incomplete_at: number | null;
    }[];
}, {}, {}>;
export declare class ListMessagesResponse extends ListMessagesResponse_base {
}
declare const ModifyMessageRequest_base: S.Class<ModifyMessageRequest, {
    metadata: S.optionalWith<typeof Metadata, {
        nullable: true;
    }>;
}, S.Struct.Encoded<{
    metadata: S.optionalWith<typeof Metadata, {
        nullable: true;
    }>;
}>, never, {
    readonly metadata?: {
        readonly [x: string]: unknown;
    } | undefined;
}, {}, {}>;
export declare class ModifyMessageRequest extends ModifyMessageRequest_base {
}
declare const DeleteMessageResponseObject_base: S.Literal<["thread.message.deleted"]>;
export declare class DeleteMessageResponseObject extends DeleteMessageResponseObject_base {
}
declare const DeleteMessageResponse_base: S.Class<DeleteMessageResponse, {
    id: typeof S.String;
    deleted: typeof S.Boolean;
    object: typeof DeleteMessageResponseObject;
}, S.Struct.Encoded<{
    id: typeof S.String;
    deleted: typeof S.Boolean;
    object: typeof DeleteMessageResponseObject;
}>, never, {
    readonly object: "thread.message.deleted";
} & {
    readonly id: string;
} & {
    readonly deleted: boolean;
}, {}, {}>;
export declare class DeleteMessageResponse extends DeleteMessageResponse_base {
}
declare const ListRunsParamsOrder_base: S.Literal<["asc", "desc"]>;
export declare class ListRunsParamsOrder extends ListRunsParamsOrder_base {
}
declare const ListRunsParams_base: S.Struct<{
    limit: S.optionalWith<typeof S.Int, {
        nullable: true;
        default: () => 20;
    }>;
    order: S.optionalWith<typeof ListRunsParamsOrder, {
        nullable: true;
        default: () => "desc";
    }>;
    after: S.optionalWith<typeof S.String, {
        nullable: true;
    }>;
    before: S.optionalWith<typeof S.String, {
        nullable: true;
    }>;
}>;
export declare class ListRunsParams extends ListRunsParams_base {
}
declare const ListRunsResponse_base: S.Class<ListRunsResponse, {
    object: typeof S.String;
    data: S.Array$<typeof RunObject>;
    first_id: typeof S.String;
    last_id: typeof S.String;
    has_more: typeof S.Boolean;
}, S.Struct.Encoded<{
    object: typeof S.String;
    data: S.Array$<typeof RunObject>;
    first_id: typeof S.String;
    last_id: typeof S.String;
    has_more: typeof S.Boolean;
}>, never, {
    readonly object: string;
} & {
    readonly first_id: string;
} & {
    readonly last_id: string;
} & {
    readonly has_more: boolean;
} & {
    readonly data: readonly RunObject[];
}, {}, {}>;
export declare class ListRunsResponse extends ListRunsResponse_base {
}
declare const CreateRunParams_base: S.Struct<{
    "include[]": S.optionalWith<S.Array$<S.Literal<["step_details.tool_calls[*].file_search.results[*].content"]>>, {
        nullable: true;
    }>;
}>;
export declare class CreateRunParams extends CreateRunParams_base {
}
declare const CreateRunRequestTruncationStrategyEnumType_base: S.Literal<["auto", "last_messages"]>;
export declare class CreateRunRequestTruncationStrategyEnumType extends CreateRunRequestTruncationStrategyEnumType_base {
}
declare const CreateRunRequestTruncationStrategy_base: S.Struct<{
    type: S.Literal<["auto", "last_messages"]>;
    last_messages: S.optionalWith<S.filter<typeof S.Int>, {
        nullable: true;
    }>;
}>;
export declare class CreateRunRequestTruncationStrategy extends CreateRunRequestTruncationStrategy_base {
}
declare const CreateRunRequestToolChoiceEnumEnum_base: S.Literal<["none", "auto", "required"]>;
export declare class CreateRunRequestToolChoiceEnumEnum extends CreateRunRequestToolChoiceEnumEnum_base {
}
declare const CreateRunRequestToolChoice_base: S.Union<[S.Literal<["none", "auto", "required"]>, typeof AssistantsNamedToolChoice]>;
export declare class CreateRunRequestToolChoice extends CreateRunRequestToolChoice_base {
}
declare const CreateRunRequest_base: S.Class<CreateRunRequest, {
    assistant_id: typeof S.String;
    model: S.optionalWith<S.Union<[typeof S.String, typeof AssistantSupportedModels]>, {
        nullable: true;
    }>;
    reasoning_effort: S.optionalWith<typeof ReasoningEffort, {
        nullable: true;
        default: () => "medium";
    }>;
    instructions: S.optionalWith<typeof S.String, {
        nullable: true;
    }>;
    additional_instructions: S.optionalWith<typeof S.String, {
        nullable: true;
    }>;
    additional_messages: S.optionalWith<S.Array$<typeof CreateMessageRequest>, {
        nullable: true;
    }>;
    tools: S.optionalWith<S.filter<S.Array$<S.Union<[typeof AssistantToolsCode, typeof AssistantToolsFileSearch, typeof AssistantToolsFunction]>>>, {
        nullable: true;
    }>;
    metadata: S.optionalWith<typeof Metadata, {
        nullable: true;
    }>;
    temperature: S.optionalWith<S.filter<S.filter<typeof S.Number>>, {
        nullable: true;
        default: () => 1;
    }>;
    top_p: S.optionalWith<S.filter<S.filter<typeof S.Number>>, {
        nullable: true;
        default: () => 1;
    }>;
    stream: S.optionalWith<typeof S.Boolean, {
        nullable: true;
    }>;
    max_prompt_tokens: S.optionalWith<S.filter<typeof S.Int>, {
        nullable: true;
    }>;
    max_completion_tokens: S.optionalWith<S.filter<typeof S.Int>, {
        nullable: true;
    }>;
    truncation_strategy: S.optionalWith<typeof CreateRunRequestTruncationStrategy, {
        nullable: true;
    }>;
    tool_choice: S.optionalWith<typeof CreateRunRequestToolChoice, {
        nullable: true;
    }>;
    parallel_tool_calls: S.optionalWith<typeof ParallelToolCalls, {
        nullable: true;
        default: () => true;
    }>;
    response_format: S.optionalWith<typeof AssistantsApiResponseFormatOption, {
        nullable: true;
    }>;
}, S.Struct.Encoded<{
    assistant_id: typeof S.String;
    model: S.optionalWith<S.Union<[typeof S.String, typeof AssistantSupportedModels]>, {
        nullable: true;
    }>;
    reasoning_effort: S.optionalWith<typeof ReasoningEffort, {
        nullable: true;
        default: () => "medium";
    }>;
    instructions: S.optionalWith<typeof S.String, {
        nullable: true;
    }>;
    additional_instructions: S.optionalWith<typeof S.String, {
        nullable: true;
    }>;
    additional_messages: S.optionalWith<S.Array$<typeof CreateMessageRequest>, {
        nullable: true;
    }>;
    tools: S.optionalWith<S.filter<S.Array$<S.Union<[typeof AssistantToolsCode, typeof AssistantToolsFileSearch, typeof AssistantToolsFunction]>>>, {
        nullable: true;
    }>;
    metadata: S.optionalWith<typeof Metadata, {
        nullable: true;
    }>;
    temperature: S.optionalWith<S.filter<S.filter<typeof S.Number>>, {
        nullable: true;
        default: () => 1;
    }>;
    top_p: S.optionalWith<S.filter<S.filter<typeof S.Number>>, {
        nullable: true;
        default: () => 1;
    }>;
    stream: S.optionalWith<typeof S.Boolean, {
        nullable: true;
    }>;
    max_prompt_tokens: S.optionalWith<S.filter<typeof S.Int>, {
        nullable: true;
    }>;
    max_completion_tokens: S.optionalWith<S.filter<typeof S.Int>, {
        nullable: true;
    }>;
    truncation_strategy: S.optionalWith<typeof CreateRunRequestTruncationStrategy, {
        nullable: true;
    }>;
    tool_choice: S.optionalWith<typeof CreateRunRequestToolChoice, {
        nullable: true;
    }>;
    parallel_tool_calls: S.optionalWith<typeof ParallelToolCalls, {
        nullable: true;
        default: () => true;
    }>;
    response_format: S.optionalWith<typeof AssistantsApiResponseFormatOption, {
        nullable: true;
    }>;
}>, never, {
    readonly model?: string | undefined;
} & {
    readonly instructions?: string | undefined;
} & {
    readonly tools?: readonly ({
        readonly type: "code_interpreter";
    } | {
        readonly type: "file_search";
        readonly file_search?: {
            readonly max_num_results?: number | undefined;
            readonly ranking_options?: {
                readonly ranker?: "auto" | "default_2024_08_21" | undefined;
                readonly score_threshold: number;
            } | undefined;
        } | undefined;
    } | {
        readonly function: {
            readonly description?: string | undefined;
            readonly name: string;
            readonly parameters?: {
                readonly [x: string]: unknown;
            } | undefined;
            readonly strict: boolean;
        };
        readonly type: "function";
    })[] | undefined;
} & {
    readonly metadata?: {
        readonly [x: string]: unknown;
    } | undefined;
} & {
    readonly temperature?: number;
} & {
    readonly top_p?: number;
} & {
    readonly response_format?: "auto" | {
        readonly type: "text";
    } | {
        readonly type: "json_object";
    } | {
        readonly type: "json_schema";
        readonly json_schema: {
            readonly description?: string | undefined;
            readonly name: string;
            readonly strict: boolean;
            readonly schema?: {
                readonly [x: string]: unknown;
            } | undefined;
        };
    } | undefined;
} & {
    readonly reasoning_effort?: "low" | "medium" | "high";
} & {
    readonly max_completion_tokens?: number | undefined;
} & {
    readonly stream?: boolean | undefined;
} & {
    readonly tool_choice?: "none" | "auto" | "required" | {
        readonly function?: {
            readonly name: string;
        } | undefined;
        readonly type: "function" | "code_interpreter" | "file_search";
    } | undefined;
} & {
    readonly parallel_tool_calls?: boolean;
} & {
    readonly assistant_id: string;
} & {
    readonly max_prompt_tokens?: number | undefined;
} & {
    readonly truncation_strategy?: {
        readonly type: "auto" | "last_messages";
        readonly last_messages?: number | undefined;
    } | undefined;
} & {
    readonly additional_instructions?: string | undefined;
} & {
    readonly additional_messages?: readonly {
        readonly content: string | readonly [{
            readonly type: "image_file";
            readonly image_file: {
                readonly detail: "auto" | "low" | "high";
                readonly file_id: string;
            };
        } | {
            readonly type: "image_url";
            readonly image_url: {
                readonly url: string;
                readonly detail: "auto" | "low" | "high";
            };
        } | {
            readonly type: "text";
            readonly text: string;
        }, ...({
            readonly type: "image_file";
            readonly image_file: {
                readonly detail: "auto" | "low" | "high";
                readonly file_id: string;
            };
        } | {
            readonly type: "image_url";
            readonly image_url: {
                readonly url: string;
                readonly detail: "auto" | "low" | "high";
            };
        } | {
            readonly type: "text";
            readonly text: string;
        })[]];
        readonly role: "assistant" | "user";
        readonly metadata?: {
            readonly [x: string]: unknown;
        } | undefined;
        readonly attachments?: readonly {
            readonly tools?: readonly ({
                readonly type: "code_interpreter";
            } | {
                readonly type: "file_search";
            })[] | undefined;
            readonly file_id?: string | undefined;
        }[] | undefined;
    }[] | undefined;
}, {}, {}>;
export declare class CreateRunRequest extends CreateRunRequest_base {
}
declare const ModifyRunRequest_base: S.Class<ModifyRunRequest, {
    metadata: S.optionalWith<typeof Metadata, {
        nullable: true;
    }>;
}, S.Struct.Encoded<{
    metadata: S.optionalWith<typeof Metadata, {
        nullable: true;
    }>;
}>, never, {
    readonly metadata?: {
        readonly [x: string]: unknown;
    } | undefined;
}, {}, {}>;
export declare class ModifyRunRequest extends ModifyRunRequest_base {
}
declare const ListRunStepsParamsOrder_base: S.Literal<["asc", "desc"]>;
export declare class ListRunStepsParamsOrder extends ListRunStepsParamsOrder_base {
}
declare const ListRunStepsParams_base: S.Struct<{
    limit: S.optionalWith<typeof S.Int, {
        nullable: true;
        default: () => 20;
    }>;
    order: S.optionalWith<typeof ListRunStepsParamsOrder, {
        nullable: true;
        default: () => "desc";
    }>;
    after: S.optionalWith<typeof S.String, {
        nullable: true;
    }>;
    before: S.optionalWith<typeof S.String, {
        nullable: true;
    }>;
    "include[]": S.optionalWith<S.Array$<S.Literal<["step_details.tool_calls[*].file_search.results[*].content"]>>, {
        nullable: true;
    }>;
}>;
export declare class ListRunStepsParams extends ListRunStepsParams_base {
}
declare const RunStepObjectObject_base: S.Literal<["thread.run.step"]>;
export declare class RunStepObjectObject extends RunStepObjectObject_base {
}
declare const RunStepObjectType_base: S.Literal<["message_creation", "tool_calls"]>;
export declare class RunStepObjectType extends RunStepObjectType_base {
}
declare const RunStepObjectStatus_base: S.Literal<["in_progress", "cancelled", "failed", "completed", "expired"]>;
export declare class RunStepObjectStatus extends RunStepObjectStatus_base {
}
declare const RunStepDetailsMessageCreationObjectType_base: S.Literal<["message_creation"]>;
export declare class RunStepDetailsMessageCreationObjectType extends RunStepDetailsMessageCreationObjectType_base {
}
declare const RunStepDetailsMessageCreationObject_base: S.Struct<{
    type: typeof RunStepDetailsMessageCreationObjectType;
    message_creation: S.Struct<{
        message_id: typeof S.String;
    }>;
}>;
export declare class RunStepDetailsMessageCreationObject extends RunStepDetailsMessageCreationObject_base {
}
declare const RunStepDetailsToolCallsObjectType_base: S.Literal<["tool_calls"]>;
export declare class RunStepDetailsToolCallsObjectType extends RunStepDetailsToolCallsObjectType_base {
}
declare const RunStepDetailsToolCallsCodeObjectType_base: S.Literal<["code_interpreter"]>;
export declare class RunStepDetailsToolCallsCodeObjectType extends RunStepDetailsToolCallsCodeObjectType_base {
}
declare const RunStepDetailsToolCallsCodeOutputLogsObjectType_base: S.Literal<["logs"]>;
export declare class RunStepDetailsToolCallsCodeOutputLogsObjectType extends RunStepDetailsToolCallsCodeOutputLogsObjectType_base {
}
declare const RunStepDetailsToolCallsCodeOutputLogsObject_base: S.Struct<{
    type: typeof RunStepDetailsToolCallsCodeOutputLogsObjectType;
    logs: typeof S.String;
}>;
export declare class RunStepDetailsToolCallsCodeOutputLogsObject extends RunStepDetailsToolCallsCodeOutputLogsObject_base {
}
declare const RunStepDetailsToolCallsCodeOutputImageObjectType_base: S.Literal<["image"]>;
export declare class RunStepDetailsToolCallsCodeOutputImageObjectType extends RunStepDetailsToolCallsCodeOutputImageObjectType_base {
}
declare const RunStepDetailsToolCallsCodeOutputImageObject_base: S.Struct<{
    type: typeof RunStepDetailsToolCallsCodeOutputImageObjectType;
    image: S.Struct<{
        file_id: typeof S.String;
    }>;
}>;
export declare class RunStepDetailsToolCallsCodeOutputImageObject extends RunStepDetailsToolCallsCodeOutputImageObject_base {
}
declare const RunStepDetailsToolCallsCodeObject_base: S.Struct<{
    id: typeof S.String;
    type: typeof RunStepDetailsToolCallsCodeObjectType;
    code_interpreter: S.Struct<{
        input: typeof S.String;
        outputs: S.Array$<S.Record$<typeof S.String, typeof S.Unknown>>;
    }>;
}>;
export declare class RunStepDetailsToolCallsCodeObject extends RunStepDetailsToolCallsCodeObject_base {
}
declare const RunStepDetailsToolCallsFileSearchObjectType_base: S.Literal<["file_search"]>;
export declare class RunStepDetailsToolCallsFileSearchObjectType extends RunStepDetailsToolCallsFileSearchObjectType_base {
}
declare const RunStepDetailsToolCallsFileSearchRankingOptionsObject_base: S.Struct<{
    ranker: typeof FileSearchRanker;
    score_threshold: S.filter<S.filter<typeof S.Number>>;
}>;
export declare class RunStepDetailsToolCallsFileSearchRankingOptionsObject extends RunStepDetailsToolCallsFileSearchRankingOptionsObject_base {
}
declare const RunStepDetailsToolCallsFileSearchResultObject_base: S.Struct<{
    file_id: typeof S.String;
    file_name: typeof S.String;
    score: S.filter<S.filter<typeof S.Number>>;
    content: S.optionalWith<S.Array$<S.Struct<{
        type: S.optionalWith<S.Literal<["text"]>, {
            nullable: true;
        }>;
        text: S.optionalWith<typeof S.String, {
            nullable: true;
        }>;
    }>>, {
        nullable: true;
    }>;
}>;
export declare class RunStepDetailsToolCallsFileSearchResultObject extends RunStepDetailsToolCallsFileSearchResultObject_base {
}
declare const RunStepDetailsToolCallsFileSearchObject_base: S.Struct<{
    id: typeof S.String;
    type: typeof RunStepDetailsToolCallsFileSearchObjectType;
    file_search: S.Struct<{
        ranking_options: S.optionalWith<typeof RunStepDetailsToolCallsFileSearchRankingOptionsObject, {
            nullable: true;
        }>;
        results: S.optionalWith<S.Array$<typeof RunStepDetailsToolCallsFileSearchResultObject>, {
            nullable: true;
        }>;
    }>;
}>;
export declare class RunStepDetailsToolCallsFileSearchObject extends RunStepDetailsToolCallsFileSearchObject_base {
}
declare const RunStepDetailsToolCallsFunctionObjectType_base: S.Literal<["function"]>;
export declare class RunStepDetailsToolCallsFunctionObjectType extends RunStepDetailsToolCallsFunctionObjectType_base {
}
declare const RunStepDetailsToolCallsFunctionObject_base: S.Struct<{
    id: typeof S.String;
    type: typeof RunStepDetailsToolCallsFunctionObjectType;
    function: S.Struct<{
        name: typeof S.String;
        arguments: typeof S.String;
        output: S.NullOr<typeof S.String>;
    }>;
}>;
export declare class RunStepDetailsToolCallsFunctionObject extends RunStepDetailsToolCallsFunctionObject_base {
}
declare const RunStepDetailsToolCallsObject_base: S.Struct<{
    type: typeof RunStepDetailsToolCallsObjectType;
    tool_calls: S.Array$<S.Union<[typeof RunStepDetailsToolCallsCodeObject, typeof RunStepDetailsToolCallsFileSearchObject, typeof RunStepDetailsToolCallsFunctionObject]>>;
}>;
export declare class RunStepDetailsToolCallsObject extends RunStepDetailsToolCallsObject_base {
}
declare const RunStepObjectLastErrorCode_base: S.Literal<["server_error", "rate_limit_exceeded"]>;
export declare class RunStepObjectLastErrorCode extends RunStepObjectLastErrorCode_base {
}
declare const RunStepCompletionUsage_base: S.Struct<{
    completion_tokens: typeof S.Int;
    prompt_tokens: typeof S.Int;
    total_tokens: typeof S.Int;
}>;
export declare class RunStepCompletionUsage extends RunStepCompletionUsage_base {
}
declare const RunStepObject_base: S.Struct<{
    id: typeof S.String;
    object: typeof RunStepObjectObject;
    created_at: typeof S.Int;
    assistant_id: typeof S.String;
    thread_id: typeof S.String;
    run_id: typeof S.String;
    type: typeof RunStepObjectType;
    status: typeof RunStepObjectStatus;
    step_details: S.Record$<typeof S.String, typeof S.Unknown>;
    last_error: S.NullOr<S.Struct<{
        code: typeof RunStepObjectLastErrorCode;
        message: typeof S.String;
    }>>;
    expired_at: S.NullOr<typeof S.Int>;
    cancelled_at: S.NullOr<typeof S.Int>;
    failed_at: S.NullOr<typeof S.Int>;
    completed_at: S.NullOr<typeof S.Int>;
    metadata: S.NullOr<typeof Metadata>;
    usage: S.NullOr<typeof RunStepCompletionUsage>;
}>;
export declare class RunStepObject extends RunStepObject_base {
}
declare const ListRunStepsResponse_base: S.Class<ListRunStepsResponse, {
    object: typeof S.String;
    data: S.Array$<typeof RunStepObject>;
    first_id: typeof S.String;
    last_id: typeof S.String;
    has_more: typeof S.Boolean;
}, S.Struct.Encoded<{
    object: typeof S.String;
    data: S.Array$<typeof RunStepObject>;
    first_id: typeof S.String;
    last_id: typeof S.String;
    has_more: typeof S.Boolean;
}>, never, {
    readonly object: string;
} & {
    readonly first_id: string;
} & {
    readonly last_id: string;
} & {
    readonly has_more: boolean;
} & {
    readonly data: readonly {
        readonly object: "thread.run.step";
        readonly type: "tool_calls" | "message_creation";
        readonly id: string;
        readonly created_at: number;
        readonly metadata: {
            readonly [x: string]: unknown;
        } | null;
        readonly status: "failed" | "in_progress" | "completed" | "expired" | "cancelled";
        readonly completed_at: number | null;
        readonly failed_at: number | null;
        readonly expired_at: number | null;
        readonly cancelled_at: number | null;
        readonly usage: {
            readonly completion_tokens: number;
            readonly prompt_tokens: number;
            readonly total_tokens: number;
        } | null;
        readonly assistant_id: string;
        readonly thread_id: string;
        readonly last_error: {
            readonly message: string;
            readonly code: "server_error" | "rate_limit_exceeded";
        } | null;
        readonly run_id: string;
        readonly step_details: {
            readonly [x: string]: unknown;
        };
    }[];
}, {}, {}>;
export declare class ListRunStepsResponse extends ListRunStepsResponse_base {
}
declare const GetRunStepParams_base: S.Struct<{
    "include[]": S.optionalWith<S.Array$<S.Literal<["step_details.tool_calls[*].file_search.results[*].content"]>>, {
        nullable: true;
    }>;
}>;
export declare class GetRunStepParams extends GetRunStepParams_base {
}
declare const SubmitToolOutputsRunRequest_base: S.Class<SubmitToolOutputsRunRequest, {
    tool_outputs: S.Array$<S.Struct<{
        tool_call_id: S.optionalWith<typeof S.String, {
            nullable: true;
        }>;
        output: S.optionalWith<typeof S.String, {
            nullable: true;
        }>;
    }>>;
    stream: S.optionalWith<typeof S.Boolean, {
        nullable: true;
    }>;
}, S.Struct.Encoded<{
    tool_outputs: S.Array$<S.Struct<{
        tool_call_id: S.optionalWith<typeof S.String, {
            nullable: true;
        }>;
        output: S.optionalWith<typeof S.String, {
            nullable: true;
        }>;
    }>>;
    stream: S.optionalWith<typeof S.Boolean, {
        nullable: true;
    }>;
}>, never, {
    readonly stream?: boolean | undefined;
} & {
    readonly tool_outputs: readonly {
        readonly tool_call_id?: string | undefined;
        readonly output?: string | undefined;
    }[];
}, {}, {}>;
export declare class SubmitToolOutputsRunRequest extends SubmitToolOutputsRunRequest_base {
}
declare const CreateUploadRequestPurpose_base: S.Literal<["assistants", "batch", "fine-tune", "vision"]>;
export declare class CreateUploadRequestPurpose extends CreateUploadRequestPurpose_base {
}
declare const CreateUploadRequest_base: S.Class<CreateUploadRequest, {
    filename: typeof S.String;
    purpose: typeof CreateUploadRequestPurpose;
    bytes: typeof S.Int;
    mime_type: typeof S.String;
}, S.Struct.Encoded<{
    filename: typeof S.String;
    purpose: typeof CreateUploadRequestPurpose;
    bytes: typeof S.Int;
    mime_type: typeof S.String;
}>, never, {
    readonly bytes: number;
} & {
    readonly filename: string;
} & {
    readonly purpose: "batch" | "assistants" | "fine-tune" | "vision";
} & {
    readonly mime_type: string;
}, {}, {}>;
export declare class CreateUploadRequest extends CreateUploadRequest_base {
}
declare const UploadStatus_base: S.Literal<["pending", "completed", "cancelled", "expired"]>;
export declare class UploadStatus extends UploadStatus_base {
}
declare const UploadObject_base: S.Literal<["upload"]>;
export declare class UploadObject extends UploadObject_base {
}
declare const UploadFileEnumObject_base: S.Literal<["file"]>;
export declare class UploadFileEnumObject extends UploadFileEnumObject_base {
}
declare const UploadFileEnumPurpose_base: S.Literal<["assistants", "assistants_output", "batch", "batch_output", "fine-tune", "fine-tune-results", "vision"]>;
export declare class UploadFileEnumPurpose extends UploadFileEnumPurpose_base {
}
declare const UploadFileEnumStatus_base: S.Literal<["uploaded", "processed", "error"]>;
export declare class UploadFileEnumStatus extends UploadFileEnumStatus_base {
}
declare const UploadFile_base: S.Struct<{
    id: typeof S.String;
    bytes: typeof S.Int;
    created_at: typeof S.Int;
    expires_at: S.optionalWith<typeof S.Int, {
        nullable: true;
    }>;
    filename: typeof S.String;
    object: S.Literal<["file"]>;
    purpose: S.Literal<["assistants", "assistants_output", "batch", "batch_output", "fine-tune", "fine-tune-results", "vision"]>;
    status: S.Literal<["uploaded", "processed", "error"]>;
    status_details: S.optionalWith<typeof S.String, {
        nullable: true;
    }>;
}>;
export declare class UploadFile extends UploadFile_base {
}
declare const Upload_base: S.Class<Upload, {
    id: typeof S.String;
    created_at: typeof S.Int;
    filename: typeof S.String;
    bytes: typeof S.Int;
    purpose: typeof S.String;
    status: typeof UploadStatus;
    expires_at: typeof S.Int;
    object: S.optionalWith<typeof UploadObject, {
        nullable: true;
    }>;
    file: S.optionalWith<typeof UploadFile, {
        nullable: true;
    }>;
}, S.Struct.Encoded<{
    id: typeof S.String;
    created_at: typeof S.Int;
    filename: typeof S.String;
    bytes: typeof S.Int;
    purpose: typeof S.String;
    status: typeof UploadStatus;
    expires_at: typeof S.Int;
    object: S.optionalWith<typeof UploadObject, {
        nullable: true;
    }>;
    file: S.optionalWith<typeof UploadFile, {
        nullable: true;
    }>;
}>, never, {
    readonly object?: "upload" | undefined;
} & {
    readonly id: string;
} & {
    readonly created_at: number;
} & {
    readonly bytes: number;
} & {
    readonly status: "completed" | "expired" | "cancelled" | "pending";
} & {
    readonly expires_at: number;
} & {
    readonly file?: {
        readonly object: "file";
        readonly id: string;
        readonly created_at: number;
        readonly bytes: number;
        readonly status: "uploaded" | "processed" | "error";
        readonly expires_at?: number | undefined;
        readonly filename: string;
        readonly purpose: "batch" | "assistants" | "assistants_output" | "batch_output" | "fine-tune" | "fine-tune-results" | "vision";
        readonly status_details?: string | undefined;
    } | undefined;
} & {
    readonly filename: string;
} & {
    readonly purpose: string;
}, {}, {}>;
export declare class Upload extends Upload_base {
}
declare const CompleteUploadRequest_base: S.Class<CompleteUploadRequest, {
    part_ids: S.Array$<typeof S.String>;
    md5: S.optionalWith<typeof S.String, {
        nullable: true;
    }>;
}, S.Struct.Encoded<{
    part_ids: S.Array$<typeof S.String>;
    md5: S.optionalWith<typeof S.String, {
        nullable: true;
    }>;
}>, never, {
    readonly part_ids: readonly string[];
} & {
    readonly md5?: string | undefined;
}, {}, {}>;
export declare class CompleteUploadRequest extends CompleteUploadRequest_base {
}
declare const UploadPartObject_base: S.Literal<["upload.part"]>;
export declare class UploadPartObject extends UploadPartObject_base {
}
declare const UploadPart_base: S.Class<UploadPart, {
    id: typeof S.String;
    created_at: typeof S.Int;
    upload_id: typeof S.String;
    object: typeof UploadPartObject;
}, S.Struct.Encoded<{
    id: typeof S.String;
    created_at: typeof S.Int;
    upload_id: typeof S.String;
    object: typeof UploadPartObject;
}>, never, {
    readonly object: "upload.part";
} & {
    readonly id: string;
} & {
    readonly created_at: number;
} & {
    readonly upload_id: string;
}, {}, {}>;
export declare class UploadPart extends UploadPart_base {
}
declare const ListVectorStoresParamsOrder_base: S.Literal<["asc", "desc"]>;
export declare class ListVectorStoresParamsOrder extends ListVectorStoresParamsOrder_base {
}
declare const ListVectorStoresParams_base: S.Struct<{
    limit: S.optionalWith<typeof S.Int, {
        nullable: true;
        default: () => 20;
    }>;
    order: S.optionalWith<typeof ListVectorStoresParamsOrder, {
        nullable: true;
        default: () => "desc";
    }>;
    after: S.optionalWith<typeof S.String, {
        nullable: true;
    }>;
    before: S.optionalWith<typeof S.String, {
        nullable: true;
    }>;
}>;
export declare class ListVectorStoresParams extends ListVectorStoresParams_base {
}
declare const VectorStoreObjectObject_base: S.Literal<["vector_store"]>;
export declare class VectorStoreObjectObject extends VectorStoreObjectObject_base {
}
declare const VectorStoreObjectStatus_base: S.Literal<["expired", "in_progress", "completed"]>;
export declare class VectorStoreObjectStatus extends VectorStoreObjectStatus_base {
}
declare const VectorStoreExpirationAfterAnchor_base: S.Literal<["last_active_at"]>;
export declare class VectorStoreExpirationAfterAnchor extends VectorStoreExpirationAfterAnchor_base {
}
declare const VectorStoreExpirationAfter_base: S.Struct<{
    anchor: typeof VectorStoreExpirationAfterAnchor;
    days: S.filter<S.filter<typeof S.Int>>;
}>;
export declare class VectorStoreExpirationAfter extends VectorStoreExpirationAfter_base {
}
declare const VectorStoreObject_base: S.Struct<{
    id: typeof S.String;
    object: typeof VectorStoreObjectObject;
    created_at: typeof S.Int;
    name: typeof S.String;
    usage_bytes: typeof S.Int;
    file_counts: S.Struct<{
        in_progress: typeof S.Int;
        completed: typeof S.Int;
        failed: typeof S.Int;
        cancelled: typeof S.Int;
        total: typeof S.Int;
    }>;
    status: typeof VectorStoreObjectStatus;
    expires_after: S.optionalWith<typeof VectorStoreExpirationAfter, {
        nullable: true;
    }>;
    expires_at: S.optionalWith<typeof S.Int, {
        nullable: true;
    }>;
    last_active_at: S.NullOr<typeof S.Int>;
    metadata: S.NullOr<typeof Metadata>;
}>;
export declare class VectorStoreObject extends VectorStoreObject_base {
}
declare const ListVectorStoresResponse_base: S.Class<ListVectorStoresResponse, {
    object: typeof S.String;
    data: S.Array$<typeof VectorStoreObject>;
    first_id: typeof S.String;
    last_id: typeof S.String;
    has_more: typeof S.Boolean;
}, S.Struct.Encoded<{
    object: typeof S.String;
    data: S.Array$<typeof VectorStoreObject>;
    first_id: typeof S.String;
    last_id: typeof S.String;
    has_more: typeof S.Boolean;
}>, never, {
    readonly object: string;
} & {
    readonly first_id: string;
} & {
    readonly last_id: string;
} & {
    readonly has_more: boolean;
} & {
    readonly data: readonly {
        readonly object: "vector_store";
        readonly name: string;
        readonly id: string;
        readonly created_at: number;
        readonly metadata: {
            readonly [x: string]: unknown;
        } | null;
        readonly status: "in_progress" | "completed" | "expired";
        readonly expires_at?: number | undefined;
        readonly usage_bytes: number;
        readonly last_active_at: number | null;
        readonly file_counts: {
            readonly failed: number;
            readonly in_progress: number;
            readonly completed: number;
            readonly cancelled: number;
            readonly total: number;
        };
        readonly expires_after?: {
            readonly anchor: "last_active_at";
            readonly days: number;
        } | undefined;
    }[];
}, {}, {}>;
export declare class ListVectorStoresResponse extends ListVectorStoresResponse_base {
}
declare const AutoChunkingStrategyRequestParamType_base: S.Literal<["auto"]>;
export declare class AutoChunkingStrategyRequestParamType extends AutoChunkingStrategyRequestParamType_base {
}
declare const AutoChunkingStrategyRequestParam_base: S.Struct<{
    type: typeof AutoChunkingStrategyRequestParamType;
}>;
export declare class AutoChunkingStrategyRequestParam extends AutoChunkingStrategyRequestParam_base {
}
declare const StaticChunkingStrategyRequestParamType_base: S.Literal<["static"]>;
export declare class StaticChunkingStrategyRequestParamType extends StaticChunkingStrategyRequestParamType_base {
}
declare const StaticChunkingStrategy_base: S.Struct<{
    max_chunk_size_tokens: S.filter<S.filter<typeof S.Int>>;
    chunk_overlap_tokens: typeof S.Int;
}>;
export declare class StaticChunkingStrategy extends StaticChunkingStrategy_base {
}
declare const StaticChunkingStrategyRequestParam_base: S.Struct<{
    type: typeof StaticChunkingStrategyRequestParamType;
    static: typeof StaticChunkingStrategy;
}>;
export declare class StaticChunkingStrategyRequestParam extends StaticChunkingStrategyRequestParam_base {
}
declare const CreateVectorStoreRequest_base: S.Class<CreateVectorStoreRequest, {
    file_ids: S.optionalWith<S.filter<S.Array$<typeof S.String>>, {
        nullable: true;
    }>;
    name: S.optionalWith<typeof S.String, {
        nullable: true;
    }>;
    expires_after: S.optionalWith<typeof VectorStoreExpirationAfter, {
        nullable: true;
    }>;
    chunking_strategy: S.optionalWith<S.Record$<typeof S.String, typeof S.Unknown>, {
        nullable: true;
    }>;
    metadata: S.optionalWith<typeof Metadata, {
        nullable: true;
    }>;
}, S.Struct.Encoded<{
    file_ids: S.optionalWith<S.filter<S.Array$<typeof S.String>>, {
        nullable: true;
    }>;
    name: S.optionalWith<typeof S.String, {
        nullable: true;
    }>;
    expires_after: S.optionalWith<typeof VectorStoreExpirationAfter, {
        nullable: true;
    }>;
    chunking_strategy: S.optionalWith<S.Record$<typeof S.String, typeof S.Unknown>, {
        nullable: true;
    }>;
    metadata: S.optionalWith<typeof Metadata, {
        nullable: true;
    }>;
}>, never, {
    readonly name?: string | undefined;
} & {
    readonly metadata?: {
        readonly [x: string]: unknown;
    } | undefined;
} & {
    readonly file_ids?: readonly string[] | undefined;
} & {
    readonly chunking_strategy?: {
        readonly [x: string]: unknown;
    } | undefined;
} & {
    readonly expires_after?: {
        readonly anchor: "last_active_at";
        readonly days: number;
    } | undefined;
}, {}, {}>;
export declare class CreateVectorStoreRequest extends CreateVectorStoreRequest_base {
}
declare const UpdateVectorStoreRequestExpiresAfterEnumAnchor_base: S.Literal<["last_active_at"]>;
export declare class UpdateVectorStoreRequestExpiresAfterEnumAnchor extends UpdateVectorStoreRequestExpiresAfterEnumAnchor_base {
}
declare const UpdateVectorStoreRequestExpiresAfter_base: S.Struct<{
    anchor: S.Literal<["last_active_at"]>;
    days: S.filter<S.filter<typeof S.Int>>;
}>;
export declare class UpdateVectorStoreRequestExpiresAfter extends UpdateVectorStoreRequestExpiresAfter_base {
}
declare const UpdateVectorStoreRequest_base: S.Class<UpdateVectorStoreRequest, {
    name: S.optionalWith<typeof S.String, {
        nullable: true;
    }>;
    expires_after: S.optionalWith<typeof UpdateVectorStoreRequestExpiresAfter, {
        nullable: true;
    }>;
    metadata: S.optionalWith<typeof Metadata, {
        nullable: true;
    }>;
}, S.Struct.Encoded<{
    name: S.optionalWith<typeof S.String, {
        nullable: true;
    }>;
    expires_after: S.optionalWith<typeof UpdateVectorStoreRequestExpiresAfter, {
        nullable: true;
    }>;
    metadata: S.optionalWith<typeof Metadata, {
        nullable: true;
    }>;
}>, never, {
    readonly name?: string | undefined;
} & {
    readonly metadata?: {
        readonly [x: string]: unknown;
    } | undefined;
} & {
    readonly expires_after?: {
        readonly anchor: "last_active_at";
        readonly days: number;
    } | undefined;
}, {}, {}>;
export declare class UpdateVectorStoreRequest extends UpdateVectorStoreRequest_base {
}
declare const DeleteVectorStoreResponseObject_base: S.Literal<["vector_store.deleted"]>;
export declare class DeleteVectorStoreResponseObject extends DeleteVectorStoreResponseObject_base {
}
declare const DeleteVectorStoreResponse_base: S.Class<DeleteVectorStoreResponse, {
    id: typeof S.String;
    deleted: typeof S.Boolean;
    object: typeof DeleteVectorStoreResponseObject;
}, S.Struct.Encoded<{
    id: typeof S.String;
    deleted: typeof S.Boolean;
    object: typeof DeleteVectorStoreResponseObject;
}>, never, {
    readonly object: "vector_store.deleted";
} & {
    readonly id: string;
} & {
    readonly deleted: boolean;
}, {}, {}>;
export declare class DeleteVectorStoreResponse extends DeleteVectorStoreResponse_base {
}
declare const ChunkingStrategyRequestParam_base: S.Record$<typeof S.String, typeof S.Unknown>;
export declare class ChunkingStrategyRequestParam extends ChunkingStrategyRequestParam_base {
}
declare const CreateVectorStoreFileBatchRequest_base: S.Class<CreateVectorStoreFileBatchRequest, {
    file_ids: S.filter<S.filter<S.Array$<typeof S.String>>>;
    chunking_strategy: S.optionalWith<typeof ChunkingStrategyRequestParam, {
        nullable: true;
    }>;
    attributes: S.optionalWith<typeof VectorStoreFileAttributes, {
        nullable: true;
    }>;
}, S.Struct.Encoded<{
    file_ids: S.filter<S.filter<S.Array$<typeof S.String>>>;
    chunking_strategy: S.optionalWith<typeof ChunkingStrategyRequestParam, {
        nullable: true;
    }>;
    attributes: S.optionalWith<typeof VectorStoreFileAttributes, {
        nullable: true;
    }>;
}>, never, {
    readonly file_ids: readonly string[];
} & {
    readonly chunking_strategy?: {
        readonly [x: string]: unknown;
    } | undefined;
} & {
    readonly attributes?: {
        readonly [x: string]: unknown;
    } | undefined;
}, {}, {}>;
export declare class CreateVectorStoreFileBatchRequest extends CreateVectorStoreFileBatchRequest_base {
}
declare const VectorStoreFileBatchObjectObject_base: S.Literal<["vector_store.files_batch"]>;
export declare class VectorStoreFileBatchObjectObject extends VectorStoreFileBatchObjectObject_base {
}
declare const VectorStoreFileBatchObjectStatus_base: S.Literal<["in_progress", "completed", "cancelled", "failed"]>;
export declare class VectorStoreFileBatchObjectStatus extends VectorStoreFileBatchObjectStatus_base {
}
declare const VectorStoreFileBatchObject_base: S.Class<VectorStoreFileBatchObject, {
    id: typeof S.String;
    object: typeof VectorStoreFileBatchObjectObject;
    created_at: typeof S.Int;
    vector_store_id: typeof S.String;
    status: typeof VectorStoreFileBatchObjectStatus;
    file_counts: S.Struct<{
        in_progress: typeof S.Int;
        completed: typeof S.Int;
        failed: typeof S.Int;
        cancelled: typeof S.Int;
        total: typeof S.Int;
    }>;
}, S.Struct.Encoded<{
    id: typeof S.String;
    object: typeof VectorStoreFileBatchObjectObject;
    created_at: typeof S.Int;
    vector_store_id: typeof S.String;
    status: typeof VectorStoreFileBatchObjectStatus;
    file_counts: S.Struct<{
        in_progress: typeof S.Int;
        completed: typeof S.Int;
        failed: typeof S.Int;
        cancelled: typeof S.Int;
        total: typeof S.Int;
    }>;
}>, never, {
    readonly object: "vector_store.files_batch";
} & {
    readonly id: string;
} & {
    readonly created_at: number;
} & {
    readonly status: "failed" | "in_progress" | "completed" | "cancelled";
} & {
    readonly file_counts: {
        readonly failed: number;
        readonly in_progress: number;
        readonly completed: number;
        readonly cancelled: number;
        readonly total: number;
    };
} & {
    readonly vector_store_id: string;
}, {}, {}>;
export declare class VectorStoreFileBatchObject extends VectorStoreFileBatchObject_base {
}
declare const ListFilesInVectorStoreBatchParamsOrder_base: S.Literal<["asc", "desc"]>;
export declare class ListFilesInVectorStoreBatchParamsOrder extends ListFilesInVectorStoreBatchParamsOrder_base {
}
declare const ListFilesInVectorStoreBatchParamsFilter_base: S.Literal<["in_progress", "completed", "failed", "cancelled"]>;
export declare class ListFilesInVectorStoreBatchParamsFilter extends ListFilesInVectorStoreBatchParamsFilter_base {
}
declare const ListFilesInVectorStoreBatchParams_base: S.Struct<{
    limit: S.optionalWith<typeof S.Int, {
        nullable: true;
        default: () => 20;
    }>;
    order: S.optionalWith<typeof ListFilesInVectorStoreBatchParamsOrder, {
        nullable: true;
        default: () => "desc";
    }>;
    after: S.optionalWith<typeof S.String, {
        nullable: true;
    }>;
    before: S.optionalWith<typeof S.String, {
        nullable: true;
    }>;
    filter: S.optionalWith<typeof ListFilesInVectorStoreBatchParamsFilter, {
        nullable: true;
    }>;
}>;
export declare class ListFilesInVectorStoreBatchParams extends ListFilesInVectorStoreBatchParams_base {
}
declare const VectorStoreFileObjectObject_base: S.Literal<["vector_store.file"]>;
export declare class VectorStoreFileObjectObject extends VectorStoreFileObjectObject_base {
}
declare const VectorStoreFileObjectStatus_base: S.Literal<["in_progress", "completed", "cancelled", "failed"]>;
export declare class VectorStoreFileObjectStatus extends VectorStoreFileObjectStatus_base {
}
declare const VectorStoreFileObjectLastErrorCode_base: S.Literal<["server_error", "unsupported_file", "invalid_file"]>;
export declare class VectorStoreFileObjectLastErrorCode extends VectorStoreFileObjectLastErrorCode_base {
}
declare const StaticChunkingStrategyResponseParamType_base: S.Literal<["static"]>;
export declare class StaticChunkingStrategyResponseParamType extends StaticChunkingStrategyResponseParamType_base {
}
declare const StaticChunkingStrategyResponseParam_base: S.Struct<{
    type: typeof StaticChunkingStrategyResponseParamType;
    static: typeof StaticChunkingStrategy;
}>;
export declare class StaticChunkingStrategyResponseParam extends StaticChunkingStrategyResponseParam_base {
}
declare const OtherChunkingStrategyResponseParamType_base: S.Literal<["other"]>;
export declare class OtherChunkingStrategyResponseParamType extends OtherChunkingStrategyResponseParamType_base {
}
declare const OtherChunkingStrategyResponseParam_base: S.Struct<{
    type: typeof OtherChunkingStrategyResponseParamType;
}>;
export declare class OtherChunkingStrategyResponseParam extends OtherChunkingStrategyResponseParam_base {
}
declare const VectorStoreFileObject_base: S.Struct<{
    id: typeof S.String;
    object: typeof VectorStoreFileObjectObject;
    usage_bytes: typeof S.Int;
    created_at: typeof S.Int;
    vector_store_id: typeof S.String;
    status: typeof VectorStoreFileObjectStatus;
    last_error: S.NullOr<S.Struct<{
        code: typeof VectorStoreFileObjectLastErrorCode;
        message: typeof S.String;
    }>>;
    chunking_strategy: S.optionalWith<S.Record$<typeof S.String, typeof S.Unknown>, {
        nullable: true;
    }>;
    attributes: S.optionalWith<typeof VectorStoreFileAttributes, {
        nullable: true;
    }>;
}>;
export declare class VectorStoreFileObject extends VectorStoreFileObject_base {
}
declare const ListVectorStoreFilesResponse_base: S.Class<ListVectorStoreFilesResponse, {
    object: typeof S.String;
    data: S.Array$<typeof VectorStoreFileObject>;
    first_id: typeof S.String;
    last_id: typeof S.String;
    has_more: typeof S.Boolean;
}, S.Struct.Encoded<{
    object: typeof S.String;
    data: S.Array$<typeof VectorStoreFileObject>;
    first_id: typeof S.String;
    last_id: typeof S.String;
    has_more: typeof S.Boolean;
}>, never, {
    readonly object: string;
} & {
    readonly first_id: string;
} & {
    readonly last_id: string;
} & {
    readonly has_more: boolean;
} & {
    readonly data: readonly {
        readonly object: "vector_store.file";
        readonly id: string;
        readonly created_at: number;
        readonly chunking_strategy?: {
            readonly [x: string]: unknown;
        } | undefined;
        readonly status: "failed" | "in_progress" | "completed" | "cancelled";
        readonly usage_bytes: number;
        readonly attributes?: {
            readonly [x: string]: unknown;
        } | undefined;
        readonly last_error: {
            readonly message: string;
            readonly code: "server_error" | "unsupported_file" | "invalid_file";
        } | null;
        readonly vector_store_id: string;
    }[];
}, {}, {}>;
export declare class ListVectorStoreFilesResponse extends ListVectorStoreFilesResponse_base {
}
declare const ListVectorStoreFilesParamsOrder_base: S.Literal<["asc", "desc"]>;
export declare class ListVectorStoreFilesParamsOrder extends ListVectorStoreFilesParamsOrder_base {
}
declare const ListVectorStoreFilesParamsFilter_base: S.Literal<["in_progress", "completed", "failed", "cancelled"]>;
export declare class ListVectorStoreFilesParamsFilter extends ListVectorStoreFilesParamsFilter_base {
}
declare const ListVectorStoreFilesParams_base: S.Struct<{
    limit: S.optionalWith<typeof S.Int, {
        nullable: true;
        default: () => 20;
    }>;
    order: S.optionalWith<typeof ListVectorStoreFilesParamsOrder, {
        nullable: true;
        default: () => "desc";
    }>;
    after: S.optionalWith<typeof S.String, {
        nullable: true;
    }>;
    before: S.optionalWith<typeof S.String, {
        nullable: true;
    }>;
    filter: S.optionalWith<typeof ListVectorStoreFilesParamsFilter, {
        nullable: true;
    }>;
}>;
export declare class ListVectorStoreFilesParams extends ListVectorStoreFilesParams_base {
}
declare const CreateVectorStoreFileRequest_base: S.Class<CreateVectorStoreFileRequest, {
    file_id: typeof S.String;
    chunking_strategy: S.optionalWith<typeof ChunkingStrategyRequestParam, {
        nullable: true;
    }>;
    attributes: S.optionalWith<typeof VectorStoreFileAttributes, {
        nullable: true;
    }>;
}, S.Struct.Encoded<{
    file_id: typeof S.String;
    chunking_strategy: S.optionalWith<typeof ChunkingStrategyRequestParam, {
        nullable: true;
    }>;
    attributes: S.optionalWith<typeof VectorStoreFileAttributes, {
        nullable: true;
    }>;
}>, never, {
    readonly chunking_strategy?: {
        readonly [x: string]: unknown;
    } | undefined;
} & {
    readonly file_id: string;
} & {
    readonly attributes?: {
        readonly [x: string]: unknown;
    } | undefined;
}, {}, {}>;
export declare class CreateVectorStoreFileRequest extends CreateVectorStoreFileRequest_base {
}
declare const UpdateVectorStoreFileAttributesRequest_base: S.Class<UpdateVectorStoreFileAttributesRequest, {
    attributes: S.NullOr<typeof VectorStoreFileAttributes>;
}, S.Struct.Encoded<{
    attributes: S.NullOr<typeof VectorStoreFileAttributes>;
}>, never, {
    readonly attributes: {
        readonly [x: string]: unknown;
    } | null;
}, {}, {}>;
export declare class UpdateVectorStoreFileAttributesRequest extends UpdateVectorStoreFileAttributesRequest_base {
}
declare const DeleteVectorStoreFileResponseObject_base: S.Literal<["vector_store.file.deleted"]>;
export declare class DeleteVectorStoreFileResponseObject extends DeleteVectorStoreFileResponseObject_base {
}
declare const DeleteVectorStoreFileResponse_base: S.Class<DeleteVectorStoreFileResponse, {
    id: typeof S.String;
    deleted: typeof S.Boolean;
    object: typeof DeleteVectorStoreFileResponseObject;
}, S.Struct.Encoded<{
    id: typeof S.String;
    deleted: typeof S.Boolean;
    object: typeof DeleteVectorStoreFileResponseObject;
}>, never, {
    readonly object: "vector_store.file.deleted";
} & {
    readonly id: string;
} & {
    readonly deleted: boolean;
}, {}, {}>;
export declare class DeleteVectorStoreFileResponse extends DeleteVectorStoreFileResponse_base {
}
declare const VectorStoreFileContentResponseObject_base: S.Literal<["vector_store.file_content.page"]>;
export declare class VectorStoreFileContentResponseObject extends VectorStoreFileContentResponseObject_base {
}
declare const VectorStoreFileContentResponse_base: S.Class<VectorStoreFileContentResponse, {
    object: typeof VectorStoreFileContentResponseObject;
    data: S.Array$<S.Struct<{
        type: S.optionalWith<typeof S.String, {
            nullable: true;
        }>;
        text: S.optionalWith<typeof S.String, {
            nullable: true;
        }>;
    }>>;
    has_more: typeof S.Boolean;
    next_page: S.NullOr<typeof S.String>;
}, S.Struct.Encoded<{
    object: typeof VectorStoreFileContentResponseObject;
    data: S.Array$<S.Struct<{
        type: S.optionalWith<typeof S.String, {
            nullable: true;
        }>;
        text: S.optionalWith<typeof S.String, {
            nullable: true;
        }>;
    }>>;
    has_more: typeof S.Boolean;
    next_page: S.NullOr<typeof S.String>;
}>, never, {
    readonly object: "vector_store.file_content.page";
} & {
    readonly has_more: boolean;
} & {
    readonly data: readonly {
        readonly type?: string | undefined;
        readonly text?: string | undefined;
    }[];
} & {
    readonly next_page: string | null;
}, {}, {}>;
export declare class VectorStoreFileContentResponse extends VectorStoreFileContentResponse_base {
}
declare const VectorStoreSearchRequestRankingOptionsRanker_base: S.Literal<["auto", "default-2024-11-15"]>;
export declare class VectorStoreSearchRequestRankingOptionsRanker extends VectorStoreSearchRequestRankingOptionsRanker_base {
}
declare const VectorStoreSearchRequest_base: S.Class<VectorStoreSearchRequest, {
    query: S.Union<[typeof S.String, S.Array$<typeof S.String>]>;
    rewrite_query: S.optionalWith<typeof S.Boolean, {
        nullable: true;
        default: () => false;
    }>;
    max_num_results: S.optionalWith<S.filter<S.filter<typeof S.Int>>, {
        nullable: true;
        default: () => 10;
    }>;
    filters: S.optionalWith<S.Union<[typeof ComparisonFilter, typeof CompoundFilter]>, {
        nullable: true;
    }>;
    ranking_options: S.optionalWith<S.Struct<{
        ranker: S.optionalWith<typeof VectorStoreSearchRequestRankingOptionsRanker, {
            nullable: true;
            default: () => "auto";
        }>;
        score_threshold: S.optionalWith<S.filter<S.filter<typeof S.Number>>, {
            nullable: true;
            default: () => 0;
        }>;
    }>, {
        nullable: true;
    }>;
}, S.Struct.Encoded<{
    query: S.Union<[typeof S.String, S.Array$<typeof S.String>]>;
    rewrite_query: S.optionalWith<typeof S.Boolean, {
        nullable: true;
        default: () => false;
    }>;
    max_num_results: S.optionalWith<S.filter<S.filter<typeof S.Int>>, {
        nullable: true;
        default: () => 10;
    }>;
    filters: S.optionalWith<S.Union<[typeof ComparisonFilter, typeof CompoundFilter]>, {
        nullable: true;
    }>;
    ranking_options: S.optionalWith<S.Struct<{
        ranker: S.optionalWith<typeof VectorStoreSearchRequestRankingOptionsRanker, {
            nullable: true;
            default: () => "auto";
        }>;
        score_threshold: S.optionalWith<S.filter<S.filter<typeof S.Number>>, {
            nullable: true;
            default: () => 0;
        }>;
    }>, {
        nullable: true;
    }>;
}>, never, {
    readonly query: string | readonly string[];
} & {
    readonly max_num_results?: number;
} & {
    readonly ranking_options?: {
        readonly ranker: "auto" | "default-2024-11-15";
        readonly score_threshold: number;
    } | undefined;
} & {
    readonly filters?: {
        readonly value: string | number | boolean;
        readonly type: "eq" | "ne" | "gt" | "gte" | "lt" | "lte";
        readonly key: string;
    } | {
        readonly type: "and" | "or";
        readonly filters: readonly ({
            readonly [x: string]: unknown;
        } | {
            readonly value: string | number | boolean;
            readonly type: "eq" | "ne" | "gt" | "gte" | "lt" | "lte";
            readonly key: string;
        })[];
    } | undefined;
} & {
    readonly rewrite_query?: boolean;
}, {}, {}>;
export declare class VectorStoreSearchRequest extends VectorStoreSearchRequest_base {
}
declare const VectorStoreSearchResultsPageObject_base: S.Literal<["vector_store.search_results.page"]>;
export declare class VectorStoreSearchResultsPageObject extends VectorStoreSearchResultsPageObject_base {
}
declare const VectorStoreSearchResultContentObjectType_base: S.Literal<["text"]>;
export declare class VectorStoreSearchResultContentObjectType extends VectorStoreSearchResultContentObjectType_base {
}
declare const VectorStoreSearchResultContentObject_base: S.Struct<{
    type: typeof VectorStoreSearchResultContentObjectType;
    text: typeof S.String;
}>;
export declare class VectorStoreSearchResultContentObject extends VectorStoreSearchResultContentObject_base {
}
declare const VectorStoreSearchResultItem_base: S.Struct<{
    file_id: typeof S.String;
    filename: typeof S.String;
    score: S.filter<S.filter<typeof S.Number>>;
    attributes: S.NullOr<typeof VectorStoreFileAttributes>;
    content: S.Array$<typeof VectorStoreSearchResultContentObject>;
}>;
export declare class VectorStoreSearchResultItem extends VectorStoreSearchResultItem_base {
}
declare const VectorStoreSearchResultsPage_base: S.Class<VectorStoreSearchResultsPage, {
    object: typeof VectorStoreSearchResultsPageObject;
    search_query: S.Array$<typeof S.String>;
    data: S.Array$<typeof VectorStoreSearchResultItem>;
    has_more: typeof S.Boolean;
    next_page: S.NullOr<typeof S.String>;
}, S.Struct.Encoded<{
    object: typeof VectorStoreSearchResultsPageObject;
    search_query: S.Array$<typeof S.String>;
    data: S.Array$<typeof VectorStoreSearchResultItem>;
    has_more: typeof S.Boolean;
    next_page: S.NullOr<typeof S.String>;
}>, never, {
    readonly object: "vector_store.search_results.page";
} & {
    readonly has_more: boolean;
} & {
    readonly data: readonly {
        readonly content: readonly {
            readonly type: "text";
            readonly text: string;
        }[];
        readonly filename: string;
        readonly file_id: string;
        readonly attributes: {
            readonly [x: string]: unknown;
        } | null;
        readonly score: number;
    }[];
} & {
    readonly next_page: string | null;
} & {
    readonly search_query: readonly string[];
}, {}, {}>;
export declare class VectorStoreSearchResultsPage extends VectorStoreSearchResultsPage_base {
}
export declare const make: (httpClient: HttpClient.HttpClient, options?: {
    readonly transformClient?: ((client: HttpClient.HttpClient) => Effect.Effect<HttpClient.HttpClient>) | undefined;
}) => Client;
export interface Client {
    readonly "listAssistants": (options: typeof ListAssistantsParams.Encoded) => Effect.Effect<typeof ListAssistantsResponse.Type, HttpClientError.HttpClientError | ParseError>;
    readonly "createAssistant": (options: typeof CreateAssistantRequest.Encoded) => Effect.Effect<typeof AssistantObject.Type, HttpClientError.HttpClientError | ParseError>;
    readonly "getAssistant": (assistantId: string) => Effect.Effect<typeof AssistantObject.Type, HttpClientError.HttpClientError | ParseError>;
    readonly "modifyAssistant": (assistantId: string, options: typeof ModifyAssistantRequest.Encoded) => Effect.Effect<typeof AssistantObject.Type, HttpClientError.HttpClientError | ParseError>;
    readonly "deleteAssistant": (assistantId: string) => Effect.Effect<typeof DeleteAssistantResponse.Type, HttpClientError.HttpClientError | ParseError>;
    readonly "createSpeech": (options: typeof CreateSpeechRequest.Encoded) => Effect.Effect<void, HttpClientError.HttpClientError | ParseError>;
    readonly "createTranscription": (options: globalThis.FormData) => Effect.Effect<typeof CreateTranscription200.Type, HttpClientError.HttpClientError | ParseError>;
    readonly "createTranslation": (options: globalThis.FormData) => Effect.Effect<typeof CreateTranslation200.Type, HttpClientError.HttpClientError | ParseError>;
    readonly "listBatches": (options: typeof ListBatchesParams.Encoded) => Effect.Effect<typeof ListBatchesResponse.Type, HttpClientError.HttpClientError | ParseError>;
    readonly "createBatch": (options: typeof CreateBatchRequest.Encoded) => Effect.Effect<typeof Batch.Type, HttpClientError.HttpClientError | ParseError>;
    readonly "retrieveBatch": (batchId: string) => Effect.Effect<typeof Batch.Type, HttpClientError.HttpClientError | ParseError>;
    readonly "cancelBatch": (batchId: string) => Effect.Effect<typeof Batch.Type, HttpClientError.HttpClientError | ParseError>;
    readonly "listChatCompletions": (options: typeof ListChatCompletionsParams.Encoded) => Effect.Effect<typeof ChatCompletionList.Type, HttpClientError.HttpClientError | ParseError>;
    readonly "createChatCompletion": (options: typeof CreateChatCompletionRequest.Encoded) => Effect.Effect<typeof CreateChatCompletionResponse.Type, HttpClientError.HttpClientError | ParseError>;
    readonly "getChatCompletion": (completionId: string) => Effect.Effect<typeof CreateChatCompletionResponse.Type, HttpClientError.HttpClientError | ParseError>;
    readonly "updateChatCompletion": (completionId: string, options: typeof UpdateChatCompletionRequest.Encoded) => Effect.Effect<typeof CreateChatCompletionResponse.Type, HttpClientError.HttpClientError | ParseError>;
    readonly "deleteChatCompletion": (completionId: string) => Effect.Effect<typeof ChatCompletionDeleted.Type, HttpClientError.HttpClientError | ParseError>;
    readonly "getChatCompletionMessages": (completionId: string, options: typeof GetChatCompletionMessagesParams.Encoded) => Effect.Effect<typeof ChatCompletionMessageList.Type, HttpClientError.HttpClientError | ParseError>;
    readonly "createCompletion": (options: typeof CreateCompletionRequest.Encoded) => Effect.Effect<typeof CreateCompletionResponse.Type, HttpClientError.HttpClientError | ParseError>;
    readonly "createEmbedding": (options: typeof CreateEmbeddingRequest.Encoded) => Effect.Effect<typeof CreateEmbeddingResponse.Type, HttpClientError.HttpClientError | ParseError>;
    readonly "listFiles": (options: typeof ListFilesParams.Encoded) => Effect.Effect<typeof ListFilesResponse.Type, HttpClientError.HttpClientError | ParseError>;
    readonly "createFile": (options: globalThis.FormData) => Effect.Effect<typeof OpenAIFile.Type, HttpClientError.HttpClientError | ParseError>;
    readonly "retrieveFile": (fileId: string) => Effect.Effect<typeof OpenAIFile.Type, HttpClientError.HttpClientError | ParseError>;
    readonly "deleteFile": (fileId: string) => Effect.Effect<typeof DeleteFileResponse.Type, HttpClientError.HttpClientError | ParseError>;
    readonly "downloadFile": (fileId: string) => Effect.Effect<typeof DownloadFile200.Type, HttpClientError.HttpClientError | ParseError>;
    readonly "listFineTuningCheckpointPermissions": (permissionId: string, options: typeof ListFineTuningCheckpointPermissionsParams.Encoded) => Effect.Effect<typeof ListFineTuningCheckpointPermissionResponse.Type, HttpClientError.HttpClientError | ParseError>;
    readonly "createFineTuningCheckpointPermission": (permissionId: string, options: typeof CreateFineTuningCheckpointPermissionRequest.Encoded) => Effect.Effect<typeof ListFineTuningCheckpointPermissionResponse.Type, HttpClientError.HttpClientError | ParseError>;
    readonly "deleteFineTuningCheckpointPermission": (permissionId: string) => Effect.Effect<typeof DeleteFineTuningCheckpointPermissionResponse.Type, HttpClientError.HttpClientError | ParseError>;
    readonly "listPaginatedFineTuningJobs": (options: typeof ListPaginatedFineTuningJobsParams.Encoded) => Effect.Effect<typeof ListPaginatedFineTuningJobsResponse.Type, HttpClientError.HttpClientError | ParseError>;
    readonly "createFineTuningJob": (options: typeof CreateFineTuningJobRequest.Encoded) => Effect.Effect<typeof FineTuningJob.Type, HttpClientError.HttpClientError | ParseError>;
    readonly "retrieveFineTuningJob": (fineTuningJobId: string) => Effect.Effect<typeof FineTuningJob.Type, HttpClientError.HttpClientError | ParseError>;
    readonly "cancelFineTuningJob": (fineTuningJobId: string) => Effect.Effect<typeof FineTuningJob.Type, HttpClientError.HttpClientError | ParseError>;
    readonly "listFineTuningJobCheckpoints": (fineTuningJobId: string, options: typeof ListFineTuningJobCheckpointsParams.Encoded) => Effect.Effect<typeof ListFineTuningJobCheckpointsResponse.Type, HttpClientError.HttpClientError | ParseError>;
    readonly "listFineTuningEvents": (fineTuningJobId: string, options: typeof ListFineTuningEventsParams.Encoded) => Effect.Effect<typeof ListFineTuningJobEventsResponse.Type, HttpClientError.HttpClientError | ParseError>;
    readonly "createImageEdit": (options: globalThis.FormData) => Effect.Effect<typeof ImagesResponse.Type, HttpClientError.HttpClientError | ParseError>;
    readonly "createImage": (options: typeof CreateImageRequest.Encoded) => Effect.Effect<typeof ImagesResponse.Type, HttpClientError.HttpClientError | ParseError>;
    readonly "createImageVariation": (options: globalThis.FormData) => Effect.Effect<typeof ImagesResponse.Type, HttpClientError.HttpClientError | ParseError>;
    readonly "listModels": () => Effect.Effect<typeof ListModelsResponse.Type, HttpClientError.HttpClientError | ParseError>;
    readonly "retrieveModel": (model: string) => Effect.Effect<typeof Model.Type, HttpClientError.HttpClientError | ParseError>;
    readonly "deleteModel": (model: string) => Effect.Effect<typeof DeleteModelResponse.Type, HttpClientError.HttpClientError | ParseError>;
    readonly "createModeration": (options: typeof CreateModerationRequest.Encoded) => Effect.Effect<typeof CreateModerationResponse.Type, HttpClientError.HttpClientError | ParseError>;
    readonly "adminApiKeysList": (options: typeof AdminApiKeysListParams.Encoded) => Effect.Effect<typeof ApiKeyList.Type, HttpClientError.HttpClientError | ParseError>;
    readonly "adminApiKeysCreate": (options: typeof AdminApiKeysCreateRequest.Encoded) => Effect.Effect<typeof AdminApiKey.Type, HttpClientError.HttpClientError | ParseError>;
    readonly "adminApiKeysGet": (keyId: string) => Effect.Effect<typeof AdminApiKey.Type, HttpClientError.HttpClientError | ParseError>;
    readonly "adminApiKeysDelete": (keyId: string) => Effect.Effect<typeof AdminApiKeysDelete200.Type, HttpClientError.HttpClientError | ParseError>;
    readonly "listAuditLogs": (options: typeof ListAuditLogsParams.Encoded) => Effect.Effect<typeof ListAuditLogsResponse.Type, HttpClientError.HttpClientError | ParseError>;
    readonly "usageCosts": (options: typeof UsageCostsParams.Encoded) => Effect.Effect<typeof UsageResponse.Type, HttpClientError.HttpClientError | ParseError>;
    readonly "listInvites": (options: typeof ListInvitesParams.Encoded) => Effect.Effect<typeof InviteListResponse.Type, HttpClientError.HttpClientError | ParseError>;
    readonly "inviteUser": (options: typeof InviteRequest.Encoded) => Effect.Effect<typeof Invite.Type, HttpClientError.HttpClientError | ParseError>;
    readonly "retrieveInvite": (inviteId: string) => Effect.Effect<typeof Invite.Type, HttpClientError.HttpClientError | ParseError>;
    readonly "deleteInvite": (inviteId: string) => Effect.Effect<typeof InviteDeleteResponse.Type, HttpClientError.HttpClientError | ParseError>;
    readonly "listProjects": (options: typeof ListProjectsParams.Encoded) => Effect.Effect<typeof ProjectListResponse.Type, HttpClientError.HttpClientError | ParseError>;
    readonly "createProject": (options: typeof ProjectCreateRequest.Encoded) => Effect.Effect<typeof Project.Type, HttpClientError.HttpClientError | ParseError>;
    readonly "retrieveProject": (projectId: string) => Effect.Effect<typeof Project.Type, HttpClientError.HttpClientError | ParseError>;
    readonly "modifyProject": (projectId: string, options: typeof ProjectUpdateRequest.Encoded) => Effect.Effect<typeof Project.Type, HttpClientError.HttpClientError | ParseError | typeof ErrorResponse.Type>;
    readonly "listProjectApiKeys": (projectId: string, options: typeof ListProjectApiKeysParams.Encoded) => Effect.Effect<typeof ProjectApiKeyListResponse.Type, HttpClientError.HttpClientError | ParseError>;
    readonly "retrieveProjectApiKey": (projectId: string, keyId: string) => Effect.Effect<typeof ProjectApiKey.Type, HttpClientError.HttpClientError | ParseError>;
    readonly "deleteProjectApiKey": (projectId: string, keyId: string) => Effect.Effect<typeof ProjectApiKeyDeleteResponse.Type, HttpClientError.HttpClientError | ParseError | typeof ErrorResponse.Type>;
    readonly "archiveProject": (projectId: string) => Effect.Effect<typeof Project.Type, HttpClientError.HttpClientError | ParseError>;
    readonly "listProjectRateLimits": (projectId: string, options: typeof ListProjectRateLimitsParams.Encoded) => Effect.Effect<typeof ProjectRateLimitListResponse.Type, HttpClientError.HttpClientError | ParseError>;
    readonly "updateProjectRateLimits": (projectId: string, rateLimitId: string, options: typeof ProjectRateLimitUpdateRequest.Encoded) => Effect.Effect<typeof ProjectRateLimit.Type, HttpClientError.HttpClientError | ParseError | typeof ErrorResponse.Type>;
    readonly "listProjectServiceAccounts": (projectId: string, options: typeof ListProjectServiceAccountsParams.Encoded) => Effect.Effect<typeof ProjectServiceAccountListResponse.Type, HttpClientError.HttpClientError | ParseError | typeof ErrorResponse.Type>;
    readonly "createProjectServiceAccount": (projectId: string, options: typeof ProjectServiceAccountCreateRequest.Encoded) => Effect.Effect<typeof ProjectServiceAccountCreateResponse.Type, HttpClientError.HttpClientError | ParseError | typeof ErrorResponse.Type>;
    readonly "retrieveProjectServiceAccount": (projectId: string, serviceAccountId: string) => Effect.Effect<typeof ProjectServiceAccount.Type, HttpClientError.HttpClientError | ParseError>;
    readonly "deleteProjectServiceAccount": (projectId: string, serviceAccountId: string) => Effect.Effect<typeof ProjectServiceAccountDeleteResponse.Type, HttpClientError.HttpClientError | ParseError>;
    readonly "listProjectUsers": (projectId: string, options: typeof ListProjectUsersParams.Encoded) => Effect.Effect<typeof ProjectUserListResponse.Type, HttpClientError.HttpClientError | ParseError | typeof ErrorResponse.Type>;
    readonly "createProjectUser": (projectId: string, options: typeof ProjectUserCreateRequest.Encoded) => Effect.Effect<typeof ProjectUser.Type, HttpClientError.HttpClientError | ParseError | typeof ErrorResponse.Type>;
    readonly "retrieveProjectUser": (projectId: string, userId: string) => Effect.Effect<typeof ProjectUser.Type, HttpClientError.HttpClientError | ParseError>;
    readonly "modifyProjectUser": (projectId: string, userId: string, options: typeof ProjectUserUpdateRequest.Encoded) => Effect.Effect<typeof ProjectUser.Type, HttpClientError.HttpClientError | ParseError | typeof ErrorResponse.Type>;
    readonly "deleteProjectUser": (projectId: string, userId: string) => Effect.Effect<typeof ProjectUserDeleteResponse.Type, HttpClientError.HttpClientError | ParseError | typeof ErrorResponse.Type>;
    readonly "usageAudioSpeeches": (options: typeof UsageAudioSpeechesParams.Encoded) => Effect.Effect<typeof UsageResponse.Type, HttpClientError.HttpClientError | ParseError>;
    readonly "usageAudioTranscriptions": (options: typeof UsageAudioTranscriptionsParams.Encoded) => Effect.Effect<typeof UsageResponse.Type, HttpClientError.HttpClientError | ParseError>;
    readonly "usageCodeInterpreterSessions": (options: typeof UsageCodeInterpreterSessionsParams.Encoded) => Effect.Effect<typeof UsageResponse.Type, HttpClientError.HttpClientError | ParseError>;
    readonly "usageCompletions": (options: typeof UsageCompletionsParams.Encoded) => Effect.Effect<typeof UsageResponse.Type, HttpClientError.HttpClientError | ParseError>;
    readonly "usageEmbeddings": (options: typeof UsageEmbeddingsParams.Encoded) => Effect.Effect<typeof UsageResponse.Type, HttpClientError.HttpClientError | ParseError>;
    readonly "usageImages": (options: typeof UsageImagesParams.Encoded) => Effect.Effect<typeof UsageResponse.Type, HttpClientError.HttpClientError | ParseError>;
    readonly "usageModerations": (options: typeof UsageModerationsParams.Encoded) => Effect.Effect<typeof UsageResponse.Type, HttpClientError.HttpClientError | ParseError>;
    readonly "usageVectorStores": (options: typeof UsageVectorStoresParams.Encoded) => Effect.Effect<typeof UsageResponse.Type, HttpClientError.HttpClientError | ParseError>;
    readonly "listUsers": (options: typeof ListUsersParams.Encoded) => Effect.Effect<typeof UserListResponse.Type, HttpClientError.HttpClientError | ParseError>;
    readonly "retrieveUser": (userId: string) => Effect.Effect<typeof User.Type, HttpClientError.HttpClientError | ParseError>;
    readonly "modifyUser": (userId: string, options: typeof UserRoleUpdateRequest.Encoded) => Effect.Effect<typeof User.Type, HttpClientError.HttpClientError | ParseError>;
    readonly "deleteUser": (userId: string) => Effect.Effect<typeof UserDeleteResponse.Type, HttpClientError.HttpClientError | ParseError>;
    readonly "createRealtimeSession": (options: typeof RealtimeSessionCreateRequest.Encoded) => Effect.Effect<typeof RealtimeSessionCreateResponse.Type, HttpClientError.HttpClientError | ParseError>;
    readonly "createRealtimeTranscriptionSession": (options: typeof RealtimeTranscriptionSessionCreateRequest.Encoded) => Effect.Effect<typeof RealtimeTranscriptionSessionCreateResponse.Type, HttpClientError.HttpClientError | ParseError>;
    readonly "createResponse": (options: typeof CreateResponse.Encoded) => Effect.Effect<typeof Response.Type, HttpClientError.HttpClientError | ParseError>;
    readonly "getResponse": (responseId: string, options: typeof GetResponseParams.Encoded) => Effect.Effect<typeof Response.Type, HttpClientError.HttpClientError | ParseError>;
    readonly "deleteResponse": (responseId: string) => Effect.Effect<void, HttpClientError.HttpClientError | ParseError | typeof Error.Type>;
    readonly "listInputItems": (responseId: string, options: typeof ListInputItemsParams.Encoded) => Effect.Effect<typeof ResponseItemList.Type, HttpClientError.HttpClientError | ParseError>;
    readonly "createThread": (options: typeof CreateThreadRequest.Encoded) => Effect.Effect<typeof ThreadObject.Type, HttpClientError.HttpClientError | ParseError>;
    readonly "createThreadAndRun": (options: typeof CreateThreadAndRunRequest.Encoded) => Effect.Effect<typeof RunObject.Type, HttpClientError.HttpClientError | ParseError>;
    readonly "getThread": (threadId: string) => Effect.Effect<typeof ThreadObject.Type, HttpClientError.HttpClientError | ParseError>;
    readonly "modifyThread": (threadId: string, options: typeof ModifyThreadRequest.Encoded) => Effect.Effect<typeof ThreadObject.Type, HttpClientError.HttpClientError | ParseError>;
    readonly "deleteThread": (threadId: string) => Effect.Effect<typeof DeleteThreadResponse.Type, HttpClientError.HttpClientError | ParseError>;
    readonly "listMessages": (threadId: string, options: typeof ListMessagesParams.Encoded) => Effect.Effect<typeof ListMessagesResponse.Type, HttpClientError.HttpClientError | ParseError>;
    readonly "createMessage": (threadId: string, options: typeof CreateMessageRequest.Encoded) => Effect.Effect<typeof MessageObject.Type, HttpClientError.HttpClientError | ParseError>;
    readonly "getMessage": (threadId: string, messageId: string) => Effect.Effect<typeof MessageObject.Type, HttpClientError.HttpClientError | ParseError>;
    readonly "modifyMessage": (threadId: string, messageId: string, options: typeof ModifyMessageRequest.Encoded) => Effect.Effect<typeof MessageObject.Type, HttpClientError.HttpClientError | ParseError>;
    readonly "deleteMessage": (threadId: string, messageId: string) => Effect.Effect<typeof DeleteMessageResponse.Type, HttpClientError.HttpClientError | ParseError>;
    readonly "listRuns": (threadId: string, options: typeof ListRunsParams.Encoded) => Effect.Effect<typeof ListRunsResponse.Type, HttpClientError.HttpClientError | ParseError>;
    readonly "createRun": (threadId: string, options: {
        readonly params: typeof CreateRunParams.Encoded;
        readonly payload: typeof CreateRunRequest.Encoded;
    }) => Effect.Effect<typeof RunObject.Type, HttpClientError.HttpClientError | ParseError>;
    readonly "getRun": (threadId: string, runId: string) => Effect.Effect<typeof RunObject.Type, HttpClientError.HttpClientError | ParseError>;
    readonly "modifyRun": (threadId: string, runId: string, options: typeof ModifyRunRequest.Encoded) => Effect.Effect<typeof RunObject.Type, HttpClientError.HttpClientError | ParseError>;
    readonly "cancelRun": (threadId: string, runId: string) => Effect.Effect<typeof RunObject.Type, HttpClientError.HttpClientError | ParseError>;
    readonly "listRunSteps": (threadId: string, runId: string, options: typeof ListRunStepsParams.Encoded) => Effect.Effect<typeof ListRunStepsResponse.Type, HttpClientError.HttpClientError | ParseError>;
    readonly "getRunStep": (threadId: string, runId: string, stepId: string, options: typeof GetRunStepParams.Encoded) => Effect.Effect<typeof RunStepObject.Type, HttpClientError.HttpClientError | ParseError>;
    readonly "submitToolOuputsToRun": (threadId: string, runId: string, options: typeof SubmitToolOutputsRunRequest.Encoded) => Effect.Effect<typeof RunObject.Type, HttpClientError.HttpClientError | ParseError>;
    readonly "createUpload": (options: typeof CreateUploadRequest.Encoded) => Effect.Effect<typeof Upload.Type, HttpClientError.HttpClientError | ParseError>;
    readonly "cancelUpload": (uploadId: string) => Effect.Effect<typeof Upload.Type, HttpClientError.HttpClientError | ParseError>;
    readonly "completeUpload": (uploadId: string, options: typeof CompleteUploadRequest.Encoded) => Effect.Effect<typeof Upload.Type, HttpClientError.HttpClientError | ParseError>;
    readonly "addUploadPart": (uploadId: string, options: globalThis.FormData) => Effect.Effect<typeof UploadPart.Type, HttpClientError.HttpClientError | ParseError>;
    readonly "listVectorStores": (options: typeof ListVectorStoresParams.Encoded) => Effect.Effect<typeof ListVectorStoresResponse.Type, HttpClientError.HttpClientError | ParseError>;
    readonly "createVectorStore": (options: typeof CreateVectorStoreRequest.Encoded) => Effect.Effect<typeof VectorStoreObject.Type, HttpClientError.HttpClientError | ParseError>;
    readonly "getVectorStore": (vectorStoreId: string) => Effect.Effect<typeof VectorStoreObject.Type, HttpClientError.HttpClientError | ParseError>;
    readonly "modifyVectorStore": (vectorStoreId: string, options: typeof UpdateVectorStoreRequest.Encoded) => Effect.Effect<typeof VectorStoreObject.Type, HttpClientError.HttpClientError | ParseError>;
    readonly "deleteVectorStore": (vectorStoreId: string) => Effect.Effect<typeof DeleteVectorStoreResponse.Type, HttpClientError.HttpClientError | ParseError>;
    readonly "createVectorStoreFileBatch": (vectorStoreId: string, options: typeof CreateVectorStoreFileBatchRequest.Encoded) => Effect.Effect<typeof VectorStoreFileBatchObject.Type, HttpClientError.HttpClientError | ParseError>;
    readonly "getVectorStoreFileBatch": (vectorStoreId: string, batchId: string) => Effect.Effect<typeof VectorStoreFileBatchObject.Type, HttpClientError.HttpClientError | ParseError>;
    readonly "cancelVectorStoreFileBatch": (vectorStoreId: string, batchId: string) => Effect.Effect<typeof VectorStoreFileBatchObject.Type, HttpClientError.HttpClientError | ParseError>;
    readonly "listFilesInVectorStoreBatch": (vectorStoreId: string, batchId: string, options: typeof ListFilesInVectorStoreBatchParams.Encoded) => Effect.Effect<typeof ListVectorStoreFilesResponse.Type, HttpClientError.HttpClientError | ParseError>;
    readonly "listVectorStoreFiles": (vectorStoreId: string, options: typeof ListVectorStoreFilesParams.Encoded) => Effect.Effect<typeof ListVectorStoreFilesResponse.Type, HttpClientError.HttpClientError | ParseError>;
    readonly "createVectorStoreFile": (vectorStoreId: string, options: typeof CreateVectorStoreFileRequest.Encoded) => Effect.Effect<typeof VectorStoreFileObject.Type, HttpClientError.HttpClientError | ParseError>;
    readonly "getVectorStoreFile": (vectorStoreId: string, fileId: string) => Effect.Effect<typeof VectorStoreFileObject.Type, HttpClientError.HttpClientError | ParseError>;
    readonly "updateVectorStoreFileAttributes": (vectorStoreId: string, fileId: string, options: typeof UpdateVectorStoreFileAttributesRequest.Encoded) => Effect.Effect<typeof VectorStoreFileObject.Type, HttpClientError.HttpClientError | ParseError>;
    readonly "deleteVectorStoreFile": (vectorStoreId: string, fileId: string) => Effect.Effect<typeof DeleteVectorStoreFileResponse.Type, HttpClientError.HttpClientError | ParseError>;
    readonly "retrieveVectorStoreFileContent": (vectorStoreId: string, fileId: string) => Effect.Effect<typeof VectorStoreFileContentResponse.Type, HttpClientError.HttpClientError | ParseError>;
    readonly "searchVectorStore": (vectorStoreId: string, options: typeof VectorStoreSearchRequest.Encoded) => Effect.Effect<typeof VectorStoreSearchResultsPage.Type, HttpClientError.HttpClientError | ParseError>;
}
export {};
//# sourceMappingURL=Generated.d.ts.map