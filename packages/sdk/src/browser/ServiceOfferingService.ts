/**
 * Browser Service Offering Service with Effect.js
 * Real-time NIP-90 marketplace operations
 */

import * as Nostr from "@openagentsinc/nostr"
import { Context, Data, Effect, Layer, Option, Ref, Schema, Stream } from "effect"
import { WebSocketService } from "./WebSocketService.js"

// Use NostrEvent type from the nostr package
type NostrEvent = Nostr.Schema.NostrEvent

// Service offering schemas
export const ServiceCapability = Schema.Struct({
  id: Schema.String,
  name: Schema.String,
  description: Schema.String,
  inputTypes: Schema.Array(Schema.Literal("url", "event", "job", "text")),
  outputType: Schema.String,
  pricing: Schema.Struct({
    basePrice: Schema.Number,
    perUnit: Schema.optional(Schema.String),
    unitLimit: Schema.optional(Schema.Number)
  })
})
export type ServiceCapability = Schema.Schema.Type<typeof ServiceCapability>

export const ServiceOffering = Schema.Struct({
  serviceId: Schema.String,
  name: Schema.String,
  description: Schema.String,
  capabilities: Schema.Array(ServiceCapability),
  provider: Schema.String,
  lightningAddress: Schema.optional(Schema.String),
  relayHints: Schema.optional(Schema.Array(Schema.String)),
  created_at: Schema.Number
})
export type ServiceOffering = Schema.Schema.Type<typeof ServiceOffering>

export const JobRequest = Schema.Struct({
  jobId: Schema.String,
  serviceId: Schema.String,
  requestKind: Schema.Number,
  input: Schema.String,
  inputType: Schema.Union(
    Schema.Literal("url"),
    Schema.Literal("event"),
    Schema.Literal("job"),
    Schema.Literal("text")
  ),
  parameters: Schema.optional(Schema.Record({ key: Schema.String, value: Schema.Unknown })),
  bidAmount: Schema.Number,
  requester: Schema.String,
  provider: Schema.String,
  created_at: Schema.Number
})
export type JobRequest = Schema.Schema.Type<typeof JobRequest>

export const JobResult = Schema.Struct({
  jobId: Schema.String,
  requestEventId: Schema.String,
  resultKind: Schema.Number,
  result: Schema.String,
  status: Schema.Union(
    Schema.Literal("payment-required"),
    Schema.Literal("processing"),
    Schema.Literal("error"),
    Schema.Literal("success"),
    Schema.Literal("partial")
  ),
  provider: Schema.String,
  computeTime: Schema.optional(Schema.Number),
  tokensUsed: Schema.optional(Schema.Number),
  confidence: Schema.optional(Schema.Number),
  created_at: Schema.Number
})
export type JobResult = Schema.Schema.Type<typeof JobResult>

// Request parameters
export const CreateJobRequestParams = Schema.Struct({
  serviceId: Schema.String,
  input: Schema.String,
  inputType: Schema.Union(
    Schema.Literal("url"),
    Schema.Literal("event"),
    Schema.Literal("job"),
    Schema.Literal("text")
  ),
  parameters: Schema.optional(Schema.Record({ key: Schema.String, value: Schema.Unknown })),
  bidAmount: Schema.Number,
  providerPubkey: Schema.String
})
export type CreateJobRequestParams = Schema.Schema.Type<typeof CreateJobRequestParams>

// Errors
export class ServiceOfferingError extends Data.TaggedError("ServiceOfferingError")<{
  reason: "connection_failed" | "invalid_service" | "request_failed" | "subscription_failed"
  message: string
  cause?: unknown
}> {}

// Nostr message types
type NostrMessage =
  | ["EVENT", string, NostrEvent]
  | ["EOSE", string]
  | ["OK", string, boolean, string]
  | ["CLOSED", string, string]
  | ["NOTICE", string]

// Service Offering Service
export class ServiceOfferingService extends Context.Tag("sdk/ServiceOfferingService")<
  ServiceOfferingService,
  {
    readonly services: Stream.Stream<ServiceOffering, ServiceOfferingError>
    readonly jobRequests: Stream.Stream<JobRequest, ServiceOfferingError>
    readonly jobResults: (jobId: string) => Stream.Stream<JobResult, ServiceOfferingError>
    readonly requestJob: (params: CreateJobRequestParams) => Effect.Effect<JobRequest, ServiceOfferingError>
  }
>() {}

// Generate subscription ID
const generateSubId = () => `sub-${Date.now()}-${Math.random().toString(36).slice(2, 9)}`

// NIP-90 job request kinds
const NIP90_JOB_REQUEST_KINDS = {
  TEXT_GENERATION: 5000,
  CODE_GENERATION: 5001,
  IMAGE_GENERATION: 5100,
  AUDIO_GENERATION: 5200,
  VIDEO_GENERATION: 5300
}

// NIP-90 job result kinds
const NIP90_JOB_RESULT_KINDS = {
  TEXT_GENERATION: 6000,
  CODE_GENERATION: 6001,
  IMAGE_GENERATION: 6100,
  AUDIO_GENERATION: 6200,
  VIDEO_GENERATION: 6300
}

// Live implementation
export const ServiceOfferingServiceLive = Layer.effect(
  ServiceOfferingService,
  Effect.gen(function*() {
    const wsService = yield* WebSocketService

    // Connect to relay
    const connection = yield* wsService.connect("ws://localhost:3003/relay").pipe(
      Effect.scoped,
      Effect.catchAll((error) =>
        Effect.fail(
          new ServiceOfferingError({
            reason: "connection_failed",
            message: error.message,
            cause: error
          })
        )
      )
    )

    // Service cache
    const serviceCache = yield* Ref.make(new Map<string, ServiceOffering>())

    // Parse Nostr messages
    const parseMessage = (data: string): Option.Option<NostrMessage> => {
      try {
        const msg = JSON.parse(data)
        if (Array.isArray(msg) && msg.length >= 2) {
          return Option.some(msg as NostrMessage)
        }
        return Option.none()
      } catch {
        return Option.none()
      }
    }

    // Subscribe to service offerings
    const subscribeToServices = Effect.gen(function*() {
      const subId = generateSubId()

      // Send subscription request for NIP-90 service offerings
      const req = JSON.stringify([
        "REQ",
        subId,
        {
          kinds: [31990], // NIP-90 service offering
          limit: 100
        }
      ])

      yield* connection.send(req).pipe(
        Effect.catchAll((error) =>
          Effect.fail(
            new ServiceOfferingError({
              reason: "subscription_failed",
              message: "Failed to subscribe to services",
              cause: error
            })
          )
        )
      )

      // Process service events
      return connection.messages.pipe(
        Stream.mapEffect((data) =>
          Effect.gen(function*() {
            const msg = parseMessage(data)
            if (Option.isNone(msg)) return Option.none()

            const message = msg.value
            if (message[0] === "EVENT" && message[1] === subId) {
              const event = message[2]
              if (event.kind === 31990) {
                try {
                  const content = JSON.parse(event.content)

                  const service: ServiceOffering = {
                    serviceId: content.serviceId,
                    name: content.name,
                    description: content.description,
                    capabilities: content.capabilities || [],
                    provider: event.pubkey,
                    lightningAddress: content.lightningAddress,
                    relayHints: content.relayHints,
                    created_at: event.created_at
                  }

                  // Update cache
                  yield* Ref.update(serviceCache, (cache) => {
                    const newCache = new Map(cache)
                    newCache.set(service.serviceId, service)
                    return newCache
                  })

                  return Option.some(service)
                } catch {
                  return Option.none()
                }
              }
            }
            return Option.none()
          })
        ),
        Stream.filter(Option.isSome),
        Stream.map((opt) => opt.value),
        Stream.catchAll((error) =>
          Stream.fail(
            new ServiceOfferingError({
              reason: "subscription_failed",
              message: error instanceof Error ? error.message : "Unknown error",
              cause: error
            })
          )
        )
      )
    })

    // Subscribe to job requests
    const subscribeToJobRequests = Effect.gen(function*() {
      const subId = generateSubId()

      // Subscribe to all job request kinds
      const req = JSON.stringify([
        "REQ",
        subId,
        {
          kinds: Object.values(NIP90_JOB_REQUEST_KINDS),
          limit: 50
        }
      ])

      yield* connection.send(req).pipe(
        Effect.catchAll((error) =>
          Effect.fail(
            new ServiceOfferingError({
              reason: "subscription_failed",
              message: "Failed to subscribe to job requests",
              cause: error
            })
          )
        )
      )

      // Process job request events
      return connection.messages.pipe(
        Stream.mapEffect((data) =>
          Effect.sync(() => {
            const msg = parseMessage(data)
            if (Option.isNone(msg)) return Option.none()

            const message = msg.value
            if (message[0] === "EVENT" && message[1] === subId) {
              const event = message[2]
              if (Object.values(NIP90_JOB_REQUEST_KINDS).includes(event.kind)) {
                try {
                  const content = JSON.parse(event.content)

                  const jobRequest: JobRequest = {
                    jobId: event.id,
                    serviceId: content.serviceId,
                    requestKind: event.kind,
                    input: content.input,
                    inputType: content.inputType || "text",
                    parameters: content.parameters,
                    bidAmount: content.bidAmount || 0,
                    requester: event.pubkey,
                    provider: event.tags.find((t: ReadonlyArray<string>) => t[0] === "p")?.[1] || "",
                    created_at: event.created_at
                  }

                  return Option.some(jobRequest)
                } catch {
                  return Option.none()
                }
              }
            }
            return Option.none()
          })
        ),
        Stream.filter(Option.isSome),
        Stream.map((opt) => opt.value),
        Stream.catchAll((error) =>
          Stream.fail(
            new ServiceOfferingError({
              reason: "subscription_failed",
              message: error instanceof Error ? error.message : "Unknown error",
              cause: error
            })
          )
        )
      )
    })

    // Subscribe to job results for a specific job
    const subscribeToJobResults = (jobId: string) =>
      Effect.gen(function*() {
        const subId = generateSubId()

        // Subscribe to job results
        const req = JSON.stringify([
          "REQ",
          subId,
          {
            kinds: Object.values(NIP90_JOB_RESULT_KINDS),
            "#e": [jobId],
            limit: 10
          }
        ])

        yield* connection.send(req).pipe(
          Effect.catchAll((error) =>
            Effect.fail(
              new ServiceOfferingError({
                reason: "subscription_failed",
                message: "Failed to subscribe to job results",
                cause: error
              })
            )
          )
        )

        // Process job result events
        return connection.messages.pipe(
          Stream.mapEffect((data) =>
            Effect.sync(() => {
              const msg = parseMessage(data)
              if (Option.isNone(msg)) return Option.none()

              const message = msg.value
              if (message[0] === "EVENT" && message[1] === subId) {
                const event = message[2]
                if (Object.values(NIP90_JOB_RESULT_KINDS).includes(event.kind)) {
                  // Check if it's for our job
                  const jobTag = event.tags.find((tag: ReadonlyArray<string>) => tag[0] === "e" && tag[1] === jobId)
                  if (jobTag) {
                    try {
                      const content = JSON.parse(event.content)

                      const jobResult: JobResult = {
                        jobId,
                        requestEventId: jobId,
                        resultKind: event.kind,
                        result: content.result || event.content,
                        status: content.status || "success",
                        provider: event.pubkey,
                        computeTime: content.computeTime,
                        tokensUsed: content.tokensUsed,
                        confidence: content.confidence,
                        created_at: event.created_at
                      }

                      return Option.some(jobResult)
                    } catch {
                      return Option.none()
                    }
                  }
                }
              }
              return Option.none()
            })
          ),
          Stream.filter(Option.isSome),
          Stream.map((opt) => opt.value),
          Stream.catchAll((error) =>
            Stream.fail(
              new ServiceOfferingError({
                reason: "subscription_failed",
                message: error instanceof Error ? error.message : "Unknown error",
                cause: error
              })
            )
          )
        )
      })

    return {
      services: Stream.unwrap(subscribeToServices),

      jobRequests: Stream.unwrap(subscribeToJobRequests),

      jobResults: (jobId: string) => Stream.unwrap(subscribeToJobResults(jobId)),

      requestJob: (_params: CreateJobRequestParams) =>
        Effect.gen(function*() {
          // For now, return error - need private key to sign events
          // This would be implemented with proper key management
          return yield* Effect.fail(
            new ServiceOfferingError({
              reason: "request_failed",
              message: "Job request requires key management implementation"
            })
          )
        })
    }
  })
)
