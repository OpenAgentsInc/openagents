/**
 * NIP-90: Data Vending Machine Service
 * Implements AI service marketplace with job request/result protocol
 * @module
 */
import { Context, Effect, Layer, Schema, Stream } from "effect";
import type { Filter, NostrEvent, PrivateKey, PublicKey } from "../core/Schema.js";
import { EventService } from "../services/EventService.js";
import { RelayService } from "../services/RelayService.js";
export declare const NIP90_JOB_REQUEST_KINDS: {
    readonly TEXT_GENERATION: 5000;
    readonly CODE_GENERATION: 5001;
    readonly IMAGE_GENERATION: 5100;
    readonly AUDIO_GENERATION: 5200;
    readonly VIDEO_GENERATION: 5300;
    readonly CODE_REVIEW: 5201;
    readonly TEXT_ANALYSIS: 5002;
    readonly DATA_ANALYSIS: 5003;
    readonly TRANSLATION: 5004;
    readonly SUMMARIZATION: 5005;
};
export declare const NIP90_JOB_RESULT_KINDS: {
    readonly TEXT_GENERATION: 6000;
    readonly CODE_GENERATION: 6001;
    readonly IMAGE_GENERATION: 6100;
    readonly AUDIO_GENERATION: 6200;
    readonly VIDEO_GENERATION: 6300;
    readonly CODE_REVIEW: 6201;
    readonly TEXT_ANALYSIS: 6002;
    readonly DATA_ANALYSIS: 6003;
    readonly TRANSLATION: 6004;
    readonly SUMMARIZATION: 6005;
};
export declare const NIP90_FEEDBACK_KIND = 7000;
declare const Nip90InvalidInputError_base: new <A extends Record<string, any> = {}>(args: import("effect/Types").Equals<A, {}> extends true ? void : { readonly [P in keyof A as P extends "_tag" ? never : P]: A[P]; }) => import("effect/Cause").YieldableError & {
    readonly _tag: "Nip90InvalidInputError";
} & Readonly<A>;
export declare class Nip90InvalidInputError extends Nip90InvalidInputError_base<{
    message: string;
    cause?: unknown;
}> {
}
declare const Nip90PublishError_base: new <A extends Record<string, any> = {}>(args: import("effect/Types").Equals<A, {}> extends true ? void : { readonly [P in keyof A as P extends "_tag" ? never : P]: A[P]; }) => import("effect/Cause").YieldableError & {
    readonly _tag: "Nip90PublishError";
} & Readonly<A>;
export declare class Nip90PublishError extends Nip90PublishError_base<{
    message: string;
    cause?: unknown;
}> {
}
declare const Nip90FetchError_base: new <A extends Record<string, any> = {}>(args: import("effect/Types").Equals<A, {}> extends true ? void : { readonly [P in keyof A as P extends "_tag" ? never : P]: A[P]; }) => import("effect/Cause").YieldableError & {
    readonly _tag: "Nip90FetchError";
} & Readonly<A>;
export declare class Nip90FetchError extends Nip90FetchError_base<{
    message: string;
    cause?: unknown;
}> {
}
declare const Nip90JobNotFoundError_base: new <A extends Record<string, any> = {}>(args: import("effect/Types").Equals<A, {}> extends true ? void : { readonly [P in keyof A as P extends "_tag" ? never : P]: A[P]; }) => import("effect/Cause").YieldableError & {
    readonly _tag: "Nip90JobNotFoundError";
} & Readonly<A>;
export declare class Nip90JobNotFoundError extends Nip90JobNotFoundError_base<{
    jobId: string;
}> {
}
declare const Nip90ServiceNotFoundError_base: new <A extends Record<string, any> = {}>(args: import("effect/Types").Equals<A, {}> extends true ? void : { readonly [P in keyof A as P extends "_tag" ? never : P]: A[P]; }) => import("effect/Cause").YieldableError & {
    readonly _tag: "Nip90ServiceNotFoundError";
} & Readonly<A>;
export declare class Nip90ServiceNotFoundError extends Nip90ServiceNotFoundError_base<{
    serviceId: string;
}> {
}
declare const Nip90PaymentError_base: new <A extends Record<string, any> = {}>(args: import("effect/Types").Equals<A, {}> extends true ? void : { readonly [P in keyof A as P extends "_tag" ? never : P]: A[P]; }) => import("effect/Cause").YieldableError & {
    readonly _tag: "Nip90PaymentError";
} & Readonly<A>;
export declare class Nip90PaymentError extends Nip90PaymentError_base<{
    message: string;
    amount?: number;
}> {
}
export declare const JobStatus: Schema.Literal<["payment-required", "processing", "error", "success", "partial"]>;
export type JobStatus = Schema.Schema.Type<typeof JobStatus>;
export declare const JobInputType: Schema.Literal<["url", "event", "job", "text"]>;
export type JobInputType = Schema.Schema.Type<typeof JobInputType>;
export declare const ServiceCapabilitySchema: Schema.Struct<{
    id: typeof Schema.String;
    name: typeof Schema.String;
    description: typeof Schema.String;
    inputTypes: Schema.Array$<Schema.Literal<["url", "event", "job", "text"]>>;
    outputType: typeof Schema.String;
    pricing: Schema.Struct<{
        basePrice: typeof Schema.Number;
        perUnit: Schema.optional<typeof Schema.String>;
        unitLimit: Schema.optional<typeof Schema.Number>;
    }>;
    parameters: Schema.optional<Schema.Record$<typeof Schema.String, typeof Schema.Unknown>>;
}>;
export type ServiceCapability = Schema.Schema.Type<typeof ServiceCapabilitySchema>;
export declare const ServiceOfferingSchema: Schema.Struct<{
    serviceId: typeof Schema.String;
    name: typeof Schema.String;
    description: typeof Schema.String;
    capabilities: Schema.Array$<Schema.Struct<{
        id: typeof Schema.String;
        name: typeof Schema.String;
        description: typeof Schema.String;
        inputTypes: Schema.Array$<Schema.Literal<["url", "event", "job", "text"]>>;
        outputType: typeof Schema.String;
        pricing: Schema.Struct<{
            basePrice: typeof Schema.Number;
            perUnit: Schema.optional<typeof Schema.String>;
            unitLimit: Schema.optional<typeof Schema.Number>;
        }>;
        parameters: Schema.optional<Schema.Record$<typeof Schema.String, typeof Schema.Unknown>>;
    }>>;
    provider: typeof Schema.String;
    lightningAddress: Schema.optional<typeof Schema.String>;
    relayHints: Schema.optional<Schema.Array$<typeof Schema.String>>;
}>;
export type ServiceOffering = Schema.Schema.Type<typeof ServiceOfferingSchema>;
export declare const JobRequestSchema: Schema.Struct<{
    jobId: typeof Schema.String;
    serviceId: typeof Schema.String;
    requestKind: typeof Schema.Number;
    input: typeof Schema.String;
    inputType: Schema.Literal<["url", "event", "job", "text"]>;
    parameters: Schema.optional<Schema.Record$<typeof Schema.String, typeof Schema.Unknown>>;
    bidAmount: typeof Schema.Number;
    requester: typeof Schema.String;
    provider: typeof Schema.String;
}>;
export type JobRequest = Schema.Schema.Type<typeof JobRequestSchema>;
export declare const JobResultSchema: Schema.Struct<{
    jobId: typeof Schema.String;
    requestEventId: typeof Schema.String;
    resultKind: typeof Schema.Number;
    result: typeof Schema.String;
    status: Schema.Literal<["payment-required", "processing", "error", "success", "partial"]>;
    provider: typeof Schema.String;
    computeTime: Schema.optional<typeof Schema.Number>;
    tokensUsed: Schema.optional<typeof Schema.Number>;
    confidence: Schema.optional<typeof Schema.Number>;
}>;
export type JobResult = Schema.Schema.Type<typeof JobResultSchema>;
export declare const JobFeedbackSchema: Schema.Struct<{
    jobId: typeof Schema.String;
    requestEventId: typeof Schema.String;
    resultEventId: Schema.optional<typeof Schema.String>;
    status: Schema.Literal<["payment-required", "processing", "error", "success", "partial"]>;
    message: typeof Schema.String;
    paymentHash: Schema.optional<typeof Schema.String>;
    amount: Schema.optional<typeof Schema.Number>;
}>;
export type JobFeedback = Schema.Schema.Type<typeof JobFeedbackSchema>;
export interface PublishServiceOfferingParams {
    serviceId: string;
    name: string;
    description: string;
    capabilities: Array<ServiceCapability>;
    lightningAddress?: string;
    relayHints?: Array<string>;
    privateKey: PrivateKey;
}
export interface RequestJobParams {
    serviceId: string;
    requestKind: number;
    input: string;
    inputType: JobInputType;
    parameters?: Record<string, unknown>;
    bidAmount: number;
    providerPubkey: string;
    privateKey: PrivateKey;
}
export interface SubmitJobResultParams {
    jobId: string;
    requestEventId: string;
    resultKind: number;
    result: string;
    status: JobStatus;
    computeTime?: number;
    tokensUsed?: number;
    confidence?: number;
    privateKey: PrivateKey;
}
export interface SubmitJobFeedbackParams {
    jobId: string;
    requestEventId: string;
    resultEventId?: string;
    status: JobStatus;
    message: string;
    paymentHash?: string;
    amount?: number;
    privateKey: PrivateKey;
}
export interface JobMonitor {
    jobId: string;
    request: JobRequest;
    result?: JobResult;
    feedback?: Array<JobFeedback>;
    status: JobStatus;
    lastUpdate: number;
}
declare const Nip90Service_base: Context.TagClass<Nip90Service, "nostr/Nip90Service", {
    /**
     * Publish a service offering (Kind 31990) to advertise AI capabilities
     */
    readonly publishServiceOffering: (params: PublishServiceOfferingParams) => Effect.Effect<NostrEvent, Nip90InvalidInputError | Nip90PublishError>;
    /**
     * Discover available services by capability or provider
     */
    readonly discoverServices: (filters?: {
        capability?: string;
        provider?: PublicKey;
        maxPrice?: number;
    }) => Effect.Effect<Array<ServiceOffering>, Nip90FetchError>;
    /**
     * Request a job from a service provider (Kind 5000-5999)
     */
    readonly requestJob: (params: RequestJobParams) => Effect.Effect<NostrEvent, Nip90InvalidInputError | Nip90PublishError>;
    /**
     * Submit job result as a service provider (Kind 6000-6999)
     */
    readonly submitJobResult: (params: SubmitJobResultParams) => Effect.Effect<NostrEvent, Nip90InvalidInputError | Nip90PublishError>;
    /**
     * Submit job feedback/status update (Kind 7000)
     */
    readonly submitJobFeedback: (params: SubmitJobFeedbackParams) => Effect.Effect<NostrEvent, Nip90InvalidInputError | Nip90PublishError>;
    /**
     * Get job status and results for a specific job
     */
    readonly getJobStatus: (jobId: string) => Effect.Effect<JobMonitor, Nip90FetchError | Nip90JobNotFoundError>;
    /**
     * Monitor job progress with real-time updates
     */
    readonly monitorJob: (jobId: string) => Stream.Stream<JobMonitor, Nip90FetchError>;
    /**
     * Get job requests for a service provider to process
     */
    readonly getJobRequests: (providerPubkey: string, filterOptions?: Partial<Filter>) => Effect.Effect<Array<JobRequest>, Nip90FetchError>;
    /**
     * Subscribe to incoming job requests for a service provider
     */
    readonly subscribeToJobRequests: (providerPubkey: string) => Stream.Stream<JobRequest, Nip90FetchError>;
}>;
export declare class Nip90Service extends Nip90Service_base {
}
export declare const Nip90ServiceLive: Layer.Layer<Nip90Service, never, EventService | RelayService>;
export {};
//# sourceMappingURL=Nip90Service.d.ts.map