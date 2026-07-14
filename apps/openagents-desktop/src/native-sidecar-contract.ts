import { Exit, Schema } from "@effect-native/core/effect"

import {
  DesktopRuntimeGatewayProtocolVersion,
  DesktopRuntimeGatewayResponseSchema,
  decodeDesktopRuntimeGatewayResponse,
  type DesktopRuntimeGatewayResponse,
} from "./runtime-gateway-contract.ts"
import { createDesktopRuntimeGateway } from "./runtime-gateway.ts"

export const DesktopNativeSidecarProtocol = "openagents.desktop.native-sidecar.v1" as const
export const DesktopNativeSidecarNodeVersion = "24.13.1" as const
export const DesktopNativeSidecarFrameLimit = 64 * 1024

const GenerationSchema = Schema.Number.check(
  Schema.isInt(),
  Schema.isGreaterThan(0),
  Schema.isLessThanOrEqualTo(Number.MAX_SAFE_INTEGER),
)
const NonceSchema = Schema.String.check(
  Schema.isMinLength(1),
  Schema.isMaxLength(80),
  Schema.isPattern(/^[A-Za-z0-9._-]+$/),
)

export const DesktopNativeSidecarBootstrapRequestSchema = Schema.Struct({
  protocol: Schema.Literal(DesktopNativeSidecarProtocol),
  generation: GenerationSchema,
  nonce: NonceSchema,
})
export type DesktopNativeSidecarBootstrapRequest =
  typeof DesktopNativeSidecarBootstrapRequestSchema.Type

export const DesktopNativeSidecarBootstrapReceiptSchema = Schema.Struct({
  protocol: Schema.Literal(DesktopNativeSidecarProtocol),
  generation: GenerationSchema,
  nonce: NonceSchema,
  pid: Schema.Number.check(Schema.isInt(), Schema.isGreaterThan(0)),
  nodeVersion: Schema.Literal(DesktopNativeSidecarNodeVersion),
  gatewayProtocolVersion: Schema.Literal(DesktopRuntimeGatewayProtocolVersion),
  requestId: Schema.Literal("native-sidecar.bootstrap"),
  response: DesktopRuntimeGatewayResponseSchema,
})
export type DesktopNativeSidecarBootstrapReceipt =
  typeof DesktopNativeSidecarBootstrapReceiptSchema.Type

const decode = <A>(schema: any, value: unknown): A | null => {
  const result = Schema.decodeUnknownExit(schema)(value, { onExcessProperty: "error" })
  return Exit.isSuccess(result) ? result.value as A : null
}

export const decodeDesktopNativeSidecarBootstrapRequest = (
  value: unknown,
): DesktopNativeSidecarBootstrapRequest | null =>
  decode(DesktopNativeSidecarBootstrapRequestSchema, value)

export const decodeDesktopNativeSidecarBootstrapReceipt = (
  value: unknown,
): DesktopNativeSidecarBootstrapReceipt | null =>
  decode(DesktopNativeSidecarBootstrapReceiptSchema, value)

export const executeDesktopNativeSidecarBootstrap = async (
  input: DesktopNativeSidecarBootstrapRequest,
  facts: Readonly<{ nodeVersion: string; pid: number }> = {
    nodeVersion: process.versions.node,
    pid: process.pid,
  },
): Promise<DesktopNativeSidecarBootstrapReceipt> => {
  const request = decodeDesktopNativeSidecarBootstrapRequest(input)
  if (request === null) throw new Error("Native sidecar bootstrap request is invalid.")
  if (facts.nodeVersion !== DesktopNativeSidecarNodeVersion) {
    throw new Error(
      `Native sidecar requires Node ${DesktopNativeSidecarNodeVersion}; observed ${facts.nodeVersion}.`,
    )
  }
  if (!Number.isSafeInteger(facts.pid) || facts.pid <= 0) {
    throw new Error("Native sidecar process identity is invalid.")
  }

  const gateway = createDesktopRuntimeGateway()
  gateway.start()
  try {
    const response = await gateway.request({
      kind: "query",
      requestId: "native-sidecar.bootstrap",
      query: { id: "runtime.bootstrap" },
    })
    const decodedResponse: DesktopRuntimeGatewayResponse | null =
      decodeDesktopRuntimeGatewayResponse(response)
    if (
      decodedResponse === null ||
      decodedResponse.kind !== "query_result" ||
      decodedResponse.requestId !== "native-sidecar.bootstrap" ||
      decodedResponse.result.kind !== "runtime.bootstrap" ||
      decodedResponse.result.lifecycle !== "ready" ||
      decodedResponse.result.protocolVersion !== DesktopRuntimeGatewayProtocolVersion
    ) {
      throw new Error("Production Desktop runtime gateway bootstrap failed closed.")
    }
    const receipt: DesktopNativeSidecarBootstrapReceipt = {
      protocol: DesktopNativeSidecarProtocol,
      generation: request.generation,
      nonce: request.nonce,
      pid: facts.pid,
      nodeVersion: DesktopNativeSidecarNodeVersion,
      gatewayProtocolVersion: DesktopRuntimeGatewayProtocolVersion,
      requestId: "native-sidecar.bootstrap",
      response: decodedResponse,
    }
    if (decodeDesktopNativeSidecarBootstrapReceipt(receipt) === null) {
      throw new Error("Native sidecar bootstrap receipt failed its output schema.")
    }
    return receipt
  } finally {
    gateway.dispose()
  }
}
