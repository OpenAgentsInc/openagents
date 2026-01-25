import { Effect } from "effect"
import type {
  GetDsrsSignatureRequest,
  GetDsrsSignatureResponse,
  ListDsrsSignaturesResponse,
} from "../gen/tauri-contracts"
import {
  GetDsrsSignatureResponseSchema,
  ListDsrsSignaturesResponseSchema,
} from "../contracts/tauri"
import { invokeWithSchema } from "./invoke.js"

export const listDsrsSignatures = Effect.fn("tauri.listDsrsSignatures")(() =>
  invokeWithSchema<ListDsrsSignaturesResponse>(
    "list_dsrs_signatures",
    undefined,
    ListDsrsSignaturesResponseSchema
  )
)

export const getDsrsSignature = Effect.fn("tauri.getDsrsSignature")(
  (payload: GetDsrsSignatureRequest) =>
    invokeWithSchema<GetDsrsSignatureResponse>(
      "get_dsrs_signature",
      payload,
      GetDsrsSignatureResponseSchema
    )
)
