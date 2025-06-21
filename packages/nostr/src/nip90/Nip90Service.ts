/**
 * NIP-90: Data Vending Machine Service
 * Implements AI service marketplace with job request/result protocol
 * @module
 */

import { Chunk, Context, Data, Effect, Layer, Option, Schema, Stream } from "effect"
import type { EventParams, Filter, NostrEvent, PrivateKey, PublicKey, SubscriptionId } from "../core/Schema.js"
import { EventService } from "../services/EventService.js"
import { RelayService } from "../services/RelayService.js"

// --- Job Request/Result Event Kinds ---
export const NIP90_JOB_REQUEST_KINDS = {
  TEXT_GENERATION: 5000,
  CODE_GENERATION: 5001,
  IMAGE_GENERATION: 5100,
  AUDIO_GENERATION: 5200,
  VIDEO_GENERATION: 5300,
  CODE_REVIEW: 5201,
  TEXT_ANALYSIS: 5002,
  DATA_ANALYSIS: 5003,
  TRANSLATION: 5004,
  SUMMARIZATION: 5005
} as const

export const NIP90_JOB_RESULT_KINDS = {
  TEXT_GENERATION: 6000,
  CODE_GENERATION: 6001,
  IMAGE_GENERATION: 6100,
  AUDIO_GENERATION: 6200,
  VIDEO_GENERATION: 6300,
  CODE_REVIEW: 6201,
  TEXT_ANALYSIS: 6002,
  DATA_ANALYSIS: 6003,
  TRANSLATION: 6004,
  SUMMARIZATION: 6005
} as const

export const NIP90_FEEDBACK_KIND = 7000

// --- Custom Error Types ---
export class Nip90InvalidInputError extends Data.TaggedError(
  "Nip90InvalidInputError"
)<{
  message: string
  cause?: unknown
}> {}

export class Nip90PublishError extends Data.TaggedError("Nip90PublishError")<{
  message: string
  cause?: unknown
}> {}

export class Nip90FetchError extends Data.TaggedError("Nip90FetchError")<{
  message: string
  cause?: unknown
}> {}

export class Nip90JobNotFoundError extends Data.TaggedError("Nip90JobNotFoundError")<{
  jobId: string
}> {}

export class Nip90ServiceNotFoundError extends Data.TaggedError("Nip90ServiceNotFoundError")<{
  serviceId: string
}> {}

export class Nip90PaymentError extends Data.TaggedError("Nip90PaymentError")<{
  message: string
  amount?: number
}> {}

// --- Job Status Types ---
export const JobStatus = Schema.Literal(
  "payment-required",
  "processing",
  "error",
  "success",
  "partial"
)
export type JobStatus = Schema.Schema.Type<typeof JobStatus>

// --- Job Input Types ---
export const JobInputType = Schema.Literal("url", "event", "job", "text")
export type JobInputType = Schema.Schema.Type<typeof JobInputType>

// --- Service Schemas ---
export const ServiceCapabilitySchema = Schema.Struct({
  id: Schema.String,
  name: Schema.String,
  description: Schema.String,
  inputTypes: Schema.Array(JobInputType),
  outputType: Schema.String,
  pricing: Schema.Struct({
    basePrice: Schema.Number, // sats
    perUnit: Schema.optional(Schema.String), // e.g., "token", "word", "file"
    unitLimit: Schema.optional(Schema.Number)
  }),
  parameters: Schema.optional(Schema.Record({
    key: Schema.String,
    value: Schema.Unknown
  }))
})
export type ServiceCapability = Schema.Schema.Type<typeof ServiceCapabilitySchema>

export const ServiceOfferingSchema = Schema.Struct({
  serviceId: Schema.String,
  name: Schema.String,
  description: Schema.String,
  capabilities: Schema.Array(ServiceCapabilitySchema),
  provider: Schema.String,
  lightningAddress: Schema.optional(Schema.String),
  relayHints: Schema.optional(Schema.Array(Schema.String))
})
export type ServiceOffering = Schema.Schema.Type<typeof ServiceOfferingSchema>

// --- Job Schemas ---
export const JobRequestSchema = Schema.Struct({
  jobId: Schema.String,
  serviceId: Schema.String,
  requestKind: Schema.Number,
  input: Schema.String, // JSON string or direct text
  inputType: JobInputType,
  parameters: Schema.optional(Schema.Record({
    key: Schema.String,
    value: Schema.Unknown
  })),
  bidAmount: Schema.Number, // sats
  requester: Schema.String,
  provider: Schema.String
})
export type JobRequest = Schema.Schema.Type<typeof JobRequestSchema>

export const JobResultSchema = Schema.Struct({
  jobId: Schema.String,
  requestEventId: Schema.String,
  resultKind: Schema.Number,
  result: Schema.String, // JSON string or direct output
  status: JobStatus,
  provider: Schema.String,
  computeTime: Schema.optional(Schema.Number), // milliseconds
  tokensUsed: Schema.optional(Schema.Number),
  confidence: Schema.optional(Schema.Number) // 0-1.0
})
export type JobResult = Schema.Schema.Type<typeof JobResultSchema>

export const JobFeedbackSchema = Schema.Struct({
  jobId: Schema.String,
  requestEventId: Schema.String,
  resultEventId: Schema.optional(Schema.String),
  status: JobStatus,
  message: Schema.String,
  paymentHash: Schema.optional(Schema.String), // Lightning payment proof
  amount: Schema.optional(Schema.Number) // sats paid
})
export type JobFeedback = Schema.Schema.Type<typeof JobFeedbackSchema>

// --- Service Method Parameters ---
export interface PublishServiceOfferingParams {
  serviceId: string
  name: string
  description: string
  capabilities: Array<ServiceCapability>
  lightningAddress?: string
  relayHints?: Array<string>
  privateKey: PrivateKey
}

export interface RequestJobParams {
  serviceId: string
  requestKind: number
  input: string
  inputType: JobInputType
  parameters?: Record<string, unknown>
  bidAmount: number // sats
  providerPubkey: string
  privateKey: PrivateKey
}

export interface SubmitJobResultParams {
  jobId: string
  requestEventId: string
  resultKind: number
  result: string
  status: JobStatus
  computeTime?: number
  tokensUsed?: number
  confidence?: number
  privateKey: PrivateKey
}

export interface SubmitJobFeedbackParams {
  jobId: string
  requestEventId: string
  resultEventId?: string
  status: JobStatus
  message: string
  rating: number
  comment?: string
  paymentHash?: string
  amount?: number
  privateKey: PrivateKey
}

export interface JobMonitor {
  jobId: string
  request: JobRequest
  result?: JobResult
  feedback?: Array<JobFeedback>
  status: JobStatus
  lastUpdate: number
}

// --- Service Tag ---
export class Nip90Service extends Context.Tag("nostr/Nip90Service")<
  Nip90Service,
  {
    /**
     * Publish a service offering (Kind 31990) to advertise AI capabilities
     */
    readonly publishServiceOffering: (
      params: PublishServiceOfferingParams
    ) => Effect.Effect<NostrEvent, Nip90InvalidInputError | Nip90PublishError>

    /**
     * Discover available services by capability or provider
     */
    readonly discoverServices: (
      filters?: {
        capability?: string
        provider?: PublicKey
        maxPrice?: number
      }
    ) => Effect.Effect<Array<ServiceOffering>, Nip90FetchError>

    /**
     * Request a job from a service provider (Kind 5000-5999)
     */
    readonly requestJob: (
      params: RequestJobParams
    ) => Effect.Effect<NostrEvent, Nip90InvalidInputError | Nip90PublishError>

    /**
     * Submit job result as a service provider (Kind 6000-6999)
     */
    readonly submitJobResult: (
      params: SubmitJobResultParams
    ) => Effect.Effect<NostrEvent, Nip90InvalidInputError | Nip90PublishError>

    /**
     * Submit job feedback/status update (Kind 7000)
     */
    readonly submitJobFeedback: (
      params: SubmitJobFeedbackParams
    ) => Effect.Effect<NostrEvent, Nip90InvalidInputError | Nip90PublishError>

    /**
     * Get job status and results for a specific job
     */
    readonly getJobStatus: (
      jobId: string
    ) => Effect.Effect<JobMonitor, Nip90FetchError | Nip90JobNotFoundError>

    /**
     * Monitor job progress with real-time updates
     */
    readonly monitorJob: (
      jobId: string
    ) => Stream.Stream<JobMonitor, Nip90FetchError>

    /**
     * Get job requests for a service provider to process
     */
    readonly getJobRequests: (
      providerPubkey: string,
      filterOptions?: Partial<Filter>
    ) => Effect.Effect<Array<JobRequest>, Nip90FetchError>

    /**
     * Subscribe to incoming job requests for a service provider
     */
    readonly subscribeToJobRequests: (
      providerPubkey: string
    ) => Stream.Stream<JobRequest, Nip90FetchError>
  }
>() {}

// --- Service Implementation ---
export const Nip90ServiceLive = Layer.effect(
  Nip90Service,
  Effect.gen(function*() {
    const eventService = yield* EventService
    const relayService = yield* RelayService

    // Default relay for NIP-90 operations
    const defaultRelay = "wss://relay.damus.io"

    const publishServiceOffering = (
      params: PublishServiceOfferingParams
    ): Effect.Effect<NostrEvent, Nip90InvalidInputError | Nip90PublishError> =>
      Effect.scoped(Effect.gen(function*() {
        // Validate inputs
        if (!params.serviceId || !params.name || params.capabilities.length === 0) {
          return yield* Effect.fail(
            new Nip90InvalidInputError({
              message: "Service ID, name, and capabilities are required"
            })
          )
        }

        // Create service offering content
        const content = JSON.stringify({
          serviceId: params.serviceId,
          name: params.name,
          description: params.description,
          capabilities: params.capabilities,
          lightningAddress: params.lightningAddress,
          relayHints: params.relayHints
        })

        // Build tags for service offering
        const tags: Array<Array<string>> = [
          ["d", params.serviceId], // NIP-33 addressable event
          ["name", params.name],
          ["about", params.description]
        ]

        // Add capability tags
        params.capabilities.forEach((cap) => {
          tags.push(["t", cap.id])
          tags.push(["k", cap.pricing.basePrice.toString()])
        })

        if (params.lightningAddress) {
          tags.push(["lud16", params.lightningAddress])
        }

        // Create Kind 31990 event (Service Offering)
        const eventParams: EventParams = {
          kind: 31990,
          tags,
          content
        }

        const event = yield* eventService.create(eventParams, params.privateKey).pipe(
          Effect.catchAll((error) =>
            Effect.fail(
              new Nip90PublishError({
                message: "Failed to create service offering event",
                cause: error
              })
            )
          )
        )

        // Publish to relay
        const connection = yield* relayService.connect("wss://relay.damus.io").pipe(
          Effect.catchAll((error) =>
            Effect.fail(
              new Nip90PublishError({
                message: "Failed to connect to relay",
                cause: error
              })
            )
          )
        )

        const published = yield* connection.publish(event).pipe(
          Effect.catchAll((error) =>
            Effect.fail(
              new Nip90PublishError({
                message: "Failed to publish service offering",
                cause: error
              })
            )
          )
        )

        if (!published) {
          return yield* Effect.fail(new Nip90PublishError({ message: "Service offering rejected by relay" }))
        }

        return event
      }))

    const discoverServices = (
      filters?: {
        capability?: string
        provider?: PublicKey
        maxPrice?: number
      }
    ): Effect.Effect<Array<ServiceOffering>, Nip90FetchError> =>
      Effect.scoped(Effect.gen(function*() {
        const connection = yield* relayService.connect("wss://relay.damus.io").pipe(
          Effect.catchAll((error) =>
            Effect.fail(
              new Nip90FetchError({
                message: "Failed to connect to relay",
                cause: error
              })
            )
          )
        )

        const filter: Filter = {
          kinds: [31990],
          ...(filters?.provider && { authors: [filters.provider] }),
          ...(filters?.capability && { "#t": [filters.capability] })
        }

        const subscription = yield* connection.subscribe("service-discovery" as SubscriptionId, [filter]).pipe(
          Effect.catchAll((error) =>
            Effect.fail(
              new Nip90FetchError({
                message: "Failed to subscribe to service offerings",
                cause: error
              })
            )
          )
        )

        const events = yield* Stream.runCollect(subscription.events).pipe(
          Effect.catchAll((error) =>
            Effect.fail(
              new Nip90FetchError({
                message: "Failed to collect service offerings",
                cause: error
              })
            )
          )
        )

        // Transform events to ServiceOffering format
        const services: Array<ServiceOffering> = Chunk.toReadonlyArray(events)
          .map((event) => {
            try {
              const content = JSON.parse(event.content)
              return {
                serviceId: content.serviceId,
                name: content.name,
                description: content.description,
                capabilities: content.capabilities,
                provider: event.pubkey,
                lightningAddress: content.lightningAddress,
                relayHints: content.relayHints
              } as ServiceOffering
            } catch {
              return null
            }
          })
          .filter((service): service is ServiceOffering => service !== null)

        // Apply price filtering if specified
        if (filters?.maxPrice) {
          return services.filter((service) =>
            service.capabilities.some((cap: any) => cap.pricing.basePrice <= filters.maxPrice!)
          )
        }

        return services
      }))

    const requestJob = (
      params: RequestJobParams
    ): Effect.Effect<NostrEvent, Nip90InvalidInputError | Nip90PublishError> =>
      Effect.scoped(Effect.gen(function*() {
        if (!params.input || params.bidAmount <= 0) {
          return yield* Effect.fail(
            new Nip90InvalidInputError({
              message: "Job input and valid bid amount are required"
            })
          )
        }

        // Create job request content
        const content = JSON.stringify({
          serviceId: params.serviceId,
          input: params.input,
          inputType: params.inputType,
          parameters: params.parameters,
          bidAmount: params.bidAmount
        })

        // Build tags for job request
        const tags: Array<Array<string>> = [
          ["p", params.providerPubkey], // Provider pubkey
          ["amount", params.bidAmount.toString(), "sats"],
          ["service", params.serviceId]
        ]

        if (params.inputType === "url") {
          tags.push(["i", params.input, "url"])
        } else if (params.inputType === "text") {
          tags.push(["i", params.input, "text"])
        }

        // Create job request event (Kind 5000-5999)
        const eventParams: EventParams = {
          kind: params.requestKind,
          tags,
          content
        }

        const event = yield* eventService.create(eventParams, params.privateKey).pipe(
          Effect.catchAll((error) =>
            Effect.fail(
              new Nip90PublishError({
                message: "Failed to create job request event",
                cause: error
              })
            )
          )
        )

        const connection = yield* relayService.connect("wss://relay.damus.io").pipe(
          Effect.catchAll((error) =>
            Effect.fail(
              new Nip90PublishError({
                message: "Failed to connect to relay",
                cause: error
              })
            )
          )
        )

        const published = yield* connection.publish(event).pipe(
          Effect.catchAll((error) =>
            Effect.fail(
              new Nip90PublishError({
                message: "Failed to publish job request",
                cause: error
              })
            )
          )
        )

        if (!published) {
          return yield* Effect.fail(new Nip90PublishError({ message: "Job request rejected by relay" }))
        }

        return event
      }))

    // Additional methods would be implemented following the same pattern...
    // For brevity, implementing key methods and leaving others as stubs

    const submitJobResult = (
      params: SubmitJobResultParams
    ): Effect.Effect<NostrEvent, Nip90InvalidInputError | Nip90PublishError> =>
      Effect.scoped(Effect.gen(function*() {
        // Implementation similar to requestJob but for result events (6000-6999)
        const content = JSON.stringify({
          jobId: params.jobId,
          result: params.result,
          status: params.status,
          computeTime: params.computeTime,
          tokensUsed: params.tokensUsed,
          confidence: params.confidence
        })

        const eventParams: EventParams = {
          kind: params.resultKind,
          tags: [
            ["e", params.requestEventId], // Reference to original request
            ["status", params.status]
          ],
          content
        }

        const event = yield* eventService.create(eventParams, params.privateKey).pipe(
          Effect.catchAll((error) =>
            Effect.fail(
              new Nip90PublishError({
                message: "Failed to create job result event",
                cause: error
              })
            )
          )
        )

        const connection = yield* relayService.connect("wss://relay.damus.io").pipe(
          Effect.catchAll((error) =>
            Effect.fail(
              new Nip90PublishError({
                message: "Failed to connect to relay",
                cause: error
              })
            )
          )
        )

        const published = yield* connection.publish(event).pipe(
          Effect.catchAll((error) =>
            Effect.fail(
              new Nip90PublishError({
                message: "Failed to publish job result",
                cause: error
              })
            )
          )
        )

        if (!published) {
          return yield* Effect.fail(new Nip90PublishError({ message: "Job result rejected by relay" }))
        }

        return event
      }))

    // Submit job feedback implementation
    const submitJobFeedback = (params: SubmitJobFeedbackParams) =>
      Effect.scoped(
        Effect.gen(function*() {
          const connection = yield* relayService.connect(defaultRelay).pipe(
            Effect.catchAll((error) =>
              Effect.fail(
                new Nip90PublishError({
                  message: "Failed to connect to relay",
                  cause: error
                })
              )
            )
          )

          // Create feedback event
          const event = yield* eventService.create(
            {
              kind: 7000, // NIP-90 feedback kind
              tags: [
                ["e", params.jobId],
                ["rating", params.rating.toString()],
                ...(params.comment ? [["comment", params.comment]] : [])
              ],
              content: params.comment || ""
            },
            params.privateKey
          ).pipe(
            Effect.catchAll((error) =>
              Effect.fail(
                new Nip90PublishError({
                  message: "Failed to create feedback event",
                  cause: error
                })
              )
            )
          )

          // Publish feedback
          const published = yield* connection.publish(event).pipe(
            Effect.catchAll((error) =>
              Effect.fail(
                new Nip90PublishError({
                  message: "Failed to publish feedback",
                  cause: error
                })
              )
            )
          )

          if (!published) {
            return yield* Effect.fail(
              new Nip90PublishError({ message: "Feedback rejected by relay" })
            )
          }

          return event
        })
      )

    const getJobStatus = (jobId: string) =>
      Effect.scoped(
        Effect.gen(function*() {
          const connection = yield* relayService.connect(defaultRelay).pipe(
            Effect.catchAll((error) =>
              Effect.fail(
                new Nip90FetchError({
                  message: "Failed to connect to relay",
                  cause: error
                })
              )
            )
          )

          // Query for job result events
          const filters: Array<Filter> = [{
            kinds: [6000, 6001, 6002, 6003], // Job result kinds
            "#e": [jobId as any],
            limit: 1
          }]

          // Subscribe and get first event
          const subscription = yield* connection.subscribe(
            ("job-status-" + jobId) as SubscriptionId,
            filters
          ).pipe(
            Effect.catchAll((error) =>
              Effect.fail(
                new Nip90FetchError({
                  message: "Failed to subscribe",
                  cause: error
                })
              )
            )
          )

          const resultEvent = yield* subscription.events.pipe(
            Stream.take(1),
            Stream.runHead
          ).pipe(
            Effect.catchAll((error) =>
              Effect.fail(
                new Nip90FetchError({
                  message: "Failed to get job status",
                  cause: error
                })
              )
            )
          )

          if (Option.isNone(resultEvent)) {
            return yield* Effect.fail(new Nip90JobNotFoundError({ jobId }))
          }

          const event = Option.getOrThrow(resultEvent)

          // Extract status from tags
          const statusTag = event.tags.find((t) => t[0] === "status")
          const status = statusTag?.[1] || "pending"

          // Transform to JobMonitor format
          const request: JobRequest = {
            jobId,
            serviceId: "",
            requestKind: 5000,
            input: "",
            inputType: "text",
            bidAmount: 0,
            requester: event.pubkey,
            provider: event.pubkey
          }

          const result: JobResult = {
            jobId,
            requestEventId: jobId,
            resultKind: event.kind,
            result: event.content,
            status: status as JobStatus,
            provider: event.pubkey
          }

          const monitor: JobMonitor = {
            jobId,
            request,
            result,
            status: status as JobStatus,
            lastUpdate: event.created_at
          }

          return monitor
        })
      )

    const monitorJob = (jobId: string) =>
      Stream.unwrapScoped(
        Effect.gen(function*() {
          const connection = yield* relayService.connect(defaultRelay).pipe(
            Effect.catchAll((error) =>
              Effect.fail(
                new Nip90FetchError({
                  message: "Failed to connect to relay",
                  cause: error
                })
              )
            )
          )

          const filters: Array<Filter> = [{
            kinds: [6000, 6001, 6002, 6003], // Job result kinds
            "#e": [jobId as any]
          }]

          const subscription = yield* connection.subscribe(
            ("monitor-" + jobId) as SubscriptionId,
            filters
          ).pipe(
            Effect.catchAll((error) =>
              Effect.fail(
                new Nip90FetchError({
                  message: "Failed to subscribe",
                  cause: error
                })
              )
            )
          )

          return subscription.events.pipe(
            Stream.map((event) => {
              // Transform event to JobMonitor
              const request: JobRequest = {
                jobId,
                serviceId: "",
                requestKind: 5000,
                input: "",
                inputType: "text",
                bidAmount: 0,
                requester: event.pubkey,
                provider: event.pubkey
              }

              const statusTag = event.tags.find((t) => t[0] === "status")
              const status = (statusTag?.[1] || "pending") as JobStatus

              const result: JobResult = {
                jobId,
                requestEventId: jobId,
                resultKind: event.kind,
                result: event.content,
                status,
                provider: event.pubkey
              }

              const monitor: JobMonitor = {
                jobId,
                request,
                result,
                status,
                lastUpdate: event.created_at
              }

              return monitor
            }),
            Stream.catchAll((error) =>
              Stream.fail(
                new Nip90FetchError({
                  message: String(error)
                })
              )
            )
          )
        })
      )

    const getJobRequests = (providerPubkey: string, filterOptions?: Partial<Filter>) =>
      Effect.scoped(
        Effect.gen(function*() {
          const connection = yield* relayService.connect(defaultRelay)
          const filters: Array<Filter> = [{
            kinds: [5000, 5001, 5002, 5003, 5999], // Job request kinds
            ...filterOptions,
            limit: filterOptions?.limit || 50
          }]

          const subscription = yield* connection.subscribe(
            ("job-requests-" + Date.now()) as SubscriptionId,
            filters
          )

          // Collect events for 2 seconds
          const events = yield* subscription.events.pipe(
            Stream.filter((event) => {
              // Filter by provider if their offering matches
              const serviceTag = event.tags.find((t) => t[0] === "service")
              return Boolean(serviceTag && serviceTag[1] === providerPubkey)
            }),
            Stream.map((event) => {
              // Transform to JobRequest
              try {
                const content = JSON.parse(event.content)
                const request: JobRequest = {
                  jobId: event.id,
                  serviceId: content.serviceId || "",
                  requestKind: event.kind,
                  input: content.input || "",
                  inputType: content.inputType || "text",
                  parameters: content.parameters,
                  bidAmount: content.bidAmount || 0,
                  requester: event.pubkey,
                  provider: providerPubkey
                }
                return request
              } catch {
                // Return default if parsing fails
                return {
                  jobId: event.id,
                  serviceId: "",
                  requestKind: event.kind,
                  input: "",
                  inputType: "text" as JobInputType,
                  bidAmount: 0,
                  requester: event.pubkey,
                  provider: providerPubkey
                }
              }
            }),
            Stream.take(100), // Take up to 100 events
            Stream.interruptAfter("2 seconds"),
            Stream.runCollect
          )

          return [...Chunk.toReadonlyArray(events)]
        })
      ).pipe(
        Effect.catchAll((error) =>
          Effect.fail(
            new Nip90FetchError({
              message: String(error)
            })
          )
        )
      )

    const subscribeToJobRequests = (providerPubkey: string) =>
      Stream.unwrapScoped(
        Effect.gen(function*() {
          const connection = yield* relayService.connect(defaultRelay).pipe(
            Effect.catchAll((error) =>
              Effect.fail(
                new Nip90FetchError({
                  message: "Failed to connect to relay",
                  cause: error
                })
              )
            )
          )

          const filters: Array<Filter> = [{
            kinds: [5000, 5001, 5002, 5003, 5999], // Job request kinds
            since: Math.floor(Date.now() / 1000)
          }]

          const subscription = yield* connection.subscribe(
            ("job-requests-stream-" + Date.now()) as SubscriptionId,
            filters
          ).pipe(
            Effect.catchAll((error) =>
              Effect.fail(
                new Nip90FetchError({
                  message: "Failed to subscribe",
                  cause: error
                })
              )
            )
          )

          return subscription.events.pipe(
            Stream.filter((event) => {
              // Filter by provider if their offering matches
              const serviceTag = event.tags.find((t) => t[0] === "service")
              return !!(serviceTag && serviceTag[1] === providerPubkey)
            }),
            Stream.map((event) => {
              // Transform to JobRequest
              try {
                const content = JSON.parse(event.content)
                const request: JobRequest = {
                  jobId: event.id,
                  serviceId: content.serviceId || "",
                  requestKind: event.kind,
                  input: content.input || "",
                  inputType: content.inputType || "text",
                  parameters: content.parameters,
                  bidAmount: content.bidAmount || 0,
                  requester: event.pubkey,
                  provider: providerPubkey
                }
                return request
              } catch {
                // Return default if parsing fails
                return {
                  jobId: event.id,
                  serviceId: "",
                  requestKind: event.kind,
                  input: "",
                  inputType: "text" as JobInputType,
                  bidAmount: 0,
                  requester: event.pubkey,
                  provider: providerPubkey
                }
              }
            }),
            Stream.catchAll((error) =>
              Stream.fail(
                new Nip90FetchError({
                  message: String(error)
                })
              )
            )
          )
        })
      )

    return {
      publishServiceOffering,
      discoverServices,
      requestJob,
      submitJobResult,
      submitJobFeedback,
      getJobStatus,
      monitorJob,
      getJobRequests,
      subscribeToJobRequests
    } as const
  })
)
