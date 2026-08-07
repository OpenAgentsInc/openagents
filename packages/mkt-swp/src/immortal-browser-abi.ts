import { Context, Effect, Layer, Schema, Semaphore } from "effect";
import { parseJsonRejectingDuplicateMembers } from "@openagentsinc/nip-mkt";

export const IMMORTAL_BROWSER_ABI_VERSION = 1 as const;
export const IMMORTAL_BROWSER_ABI_SCHEMA = "openagents.immortal.mkt-swp.browser-abi.v1" as const;
export const IMMORTAL_BROWSER_SOURCE_REVISION = "d62a4f7c6c34a11d191fe78316fd8d4ce4da1d34" as const;
export const IMMORTAL_REQUESTER_API_SHA256 =
  "bf52fda5f4d349fbbe195e4cff58af59a3930e1ee8ab1f1413b6338ba44fb3a8" as const;
export const IMMORTAL_BROWSER_MAX_REQUEST_BYTES = 2 * 1024 * 1024;
export const IMMORTAL_BROWSER_MAX_RESPONSE_BYTES = 8 * 1024 * 1024;

export const IMMORTAL_BROWSER_OPERATIONS = [
  "metadata",
  "validate_offering",
  "validate_delivery",
  "verify_signed",
  "requester_rfq",
  "requester_order",
  "requester_contract_draft",
  "requester_contract",
  "requester_cancel",
  "requester_close",
  "exit_package_inspect",
  "session_create",
  "session_ingest",
  "session_restore",
  "prepare_funding_request",
  "verify_before_fund",
] as const;

export const ImmortalBrowserOperationSchema = Schema.Literals(IMMORTAL_BROWSER_OPERATIONS);
export type ImmortalBrowserOperation = typeof ImmortalBrowserOperationSchema.Type;

const LowerHex64Schema = Schema.String.check(Schema.isPattern(/^[0-9a-f]{64}$/));
const NonNegativeSafeIntegerSchema = Schema.Number.check(
  Schema.isInt(),
  Schema.isGreaterThanOrEqualTo(0),
  Schema.isLessThanOrEqualTo(Number.MAX_SAFE_INTEGER),
);
const UInt16Schema = NonNegativeSafeIntegerSchema.pipe(
  Schema.check(Schema.isLessThanOrEqualTo(65_535)),
);
const UInt32Schema = NonNegativeSafeIntegerSchema.pipe(
  Schema.check(Schema.isLessThanOrEqualTo(4_294_967_295)),
);
const ByteSchema = UInt16Schema.pipe(Schema.check(Schema.isLessThanOrEqualTo(255)));
const ByteArraySchema = Schema.Array(ByteSchema);
const LowerHexSchema = Schema.String.check(Schema.isPattern(/^(?:[0-9a-f]{2})+$/));
const DecimalStringSchema = Schema.String.check(Schema.isPattern(/^(0|[1-9][0-9]*)$/));
const NostrTagSchema = Schema.NonEmptyArray(Schema.String);

export const ImmortalBrowserMetadataSchema = Schema.Struct({
  schema: Schema.Literal(IMMORTAL_BROWSER_ABI_SCHEMA),
  abi_version: Schema.Literal(IMMORTAL_BROWSER_ABI_VERSION),
  source_revision: Schema.Literal(IMMORTAL_BROWSER_SOURCE_REVISION),
  requester_api_sha256: Schema.Literal(IMMORTAL_REQUESTER_API_SHA256),
  maximum_request_bytes: Schema.Literal(IMMORTAL_BROWSER_MAX_REQUEST_BYTES),
  maximum_response_bytes: Schema.Literal(IMMORTAL_BROWSER_MAX_RESPONSE_BYTES),
  operations: Schema.Array(ImmortalBrowserOperationSchema),
  custody: Schema.Literal("host_owned"),
});
export type ImmortalBrowserMetadata = typeof ImmortalBrowserMetadataSchema.Type;

export const ImmortalSigningRequestSchema = Schema.Struct({
  pubkey: LowerHex64Schema,
  created_at: NonNegativeSafeIntegerSchema,
  kind: UInt16Schema,
  tags: Schema.Array(NostrTagSchema),
  content: Schema.String,
  expected_event_id: LowerHex64Schema,
});
export type ImmortalSigningRequest = typeof ImmortalSigningRequestSchema.Type;

export const ImmortalNostrEventSchema = Schema.Struct({
  id: LowerHex64Schema,
  pubkey: LowerHex64Schema,
  created_at: NonNegativeSafeIntegerSchema,
  kind: UInt16Schema,
  tags: Schema.Array(NostrTagSchema),
  content: Schema.String,
  sig: Schema.String.check(Schema.isPattern(/^[0-9a-f]{128}$/)),
});
export type ImmortalNostrEvent = typeof ImmortalNostrEventSchema.Type;

export const ImmortalSignedRecordDeliverySchema = Schema.Struct({
  event_id: LowerHex64Schema,
  raw_signed_event: ByteArraySchema,
  raw_wrap_event: Schema.NullOr(ByteArraySchema),
  wrap_event_id: Schema.NullOr(LowerHex64Schema),
  sender_pubkey: LowerHex64Schema,
  observed_at: NonNegativeSafeIntegerSchema,
  provenance: Schema.Literals(["locally_signed", "direct", "gift_wrap"]),
});
export type ImmortalSignedRecordDelivery = typeof ImmortalSignedRecordDeliverySchema.Type;

const ImmortalRequesterFeeViewSchema = Schema.Struct({
  fee_bps: DecimalStringSchema,
  provider_fee: DecimalStringSchema,
  miner_fee_budget: DecimalStringSchema,
  lightning_routing_fee_budget: DecimalStringSchema,
  maximum_total_fee: DecimalStringSchema,
  fee_payer: Schema.NonEmptyString,
});

const ImmortalRequesterPriceFeedViewSchema = Schema.Struct({
  url: Schema.NonEmptyString,
  value_pointer: Schema.String,
  observed_value: Schema.NonEmptyString,
  response_sha256: LowerHex64Schema,
  observed_at: NonNegativeSafeIntegerSchema,
  max_age_seconds: NonNegativeSafeIntegerSchema,
});

const ImmortalRequesterQuoteViewSchema = Schema.Struct({
  rfq_id: LowerHex64Schema,
  quote_id: LowerHex64Schema,
  provider_pubkey: LowerHex64Schema,
  quote_class: Schema.NonEmptyString,
  reservation_class: Schema.NonEmptyString,
  swap_type: Schema.Literals(["submarine", "reverse", "chain"]),
  input_asset_id: Schema.NonEmptyString,
  output_asset_id: Schema.NonEmptyString,
  input_amount: DecimalStringSchema,
  output_amount: DecimalStringSchema,
  amount_equation: Schema.NonEmptyString,
  rounding: Schema.NonEmptyString,
  clock_skew_seconds: DecimalStringSchema,
  expires_at: NonNegativeSafeIntegerSchema,
  effective_acceptance_deadline: NonNegativeSafeIntegerSchema,
  fees: ImmortalRequesterFeeViewSchema,
  price_feed: Schema.NullOr(ImmortalRequesterPriceFeedViewSchema),
});

const ImmortalRequesterTimelineEntrySchema = Schema.Struct({
  event_id: LowerHex64Schema,
  author: Schema.Literals(["requester", "provider"]),
  kind: Schema.Literals(["rfq", "quote", "order", "contract", "status", "cancel", "close"]),
  created_at: NonNegativeSafeIntegerSchema,
  sequence: Schema.NullOr(NonNegativeSafeIntegerSchema),
  state: Schema.NullOr(Schema.String),
  causal_event_ids: Schema.Array(LowerHex64Schema),
  conflict: Schema.NullOr(Schema.String),
});

export const ImmortalRequesterSessionViewSchema = Schema.Struct({
  schema: Schema.Literal("openagents.mkt-swp.requester-session-view.v1"),
  session_id: LowerHex64Schema,
  quote: ImmortalRequesterQuoteViewSchema,
  timeline: Schema.Array(ImmortalRequesterTimelineEntrySchema),
  verification: Schema.Struct({
    state: Schema.Literals([
      "quote_verified",
      "order_verified",
      "awaiting_provider_contract",
      "contract_terms_verified",
      "terminal_verified",
    ]),
    local_verification_required: Schema.Boolean,
    funding_authorized: Schema.Boolean,
    status_gaps: Schema.Array(Schema.String),
    status_forks: Schema.Array(Schema.String),
    invalid_status_claims: Schema.Array(Schema.String),
  }),
  terminal: Schema.Struct({
    claimed_state: Schema.Literals([
      "open",
      "completed",
      "refunded",
      "cancelled",
      "rejected",
      "expired",
      "failed",
      "disputed",
      "unresolved",
      "conflicted",
    ]),
    canonical_close_id: Schema.NullOr(LowerHex64Schema),
    close_event_ids: Schema.Array(LowerHex64Schema),
    principal_unresolved: Schema.NullOr(Schema.String),
    loss_accounting_complete: Schema.Boolean,
    local_effects_verified: Schema.Boolean,
    watch_terminal: Schema.Boolean,
  }),
  deliveries: Schema.Array(ImmortalSignedRecordDeliverySchema),
});
export type ImmortalRequesterSessionView = typeof ImmortalRequesterSessionViewSchema.Type;

export const ImmortalSessionResultSchema = Schema.Struct({
  snapshot_json_hex: Schema.String.check(Schema.isPattern(/^(?:[0-9a-f]{2})+$/)),
  view: ImmortalRequesterSessionViewSchema,
});
export type ImmortalSessionResult = typeof ImmortalSessionResultSchema.Type;

export const ImmortalSessionIngestResultSchema = Schema.Struct({
  ...ImmortalSessionResultSchema.fields,
  ingested_records: NonNegativeSafeIntegerSchema,
});
export type ImmortalSessionIngestResult = typeof ImmortalSessionIngestResultSchema.Type;

const ImmortalLiquidFundingVerificationInputSchema = Schema.Struct({
  raw_transaction: LowerHexSchema,
  trusted_unblind_transaction: Schema.NullOr(LowerHexSchema),
  transaction_sha256: LowerHex64Schema,
  output_index: UInt32Schema,
  asset_id: Schema.NonEmptyString,
  amount: DecimalStringSchema,
  script_pubkey: LowerHexSchema,
  taproot_internal_key: LowerHex64Schema,
  taproot_merkle_root: Schema.NullOr(LowerHex64Schema),
  confidentiality: Schema.Literals(["explicit", "confidential"]),
  minimum_confirmations: UInt32Schema,
  replacement_policy: Schema.NonEmptyString,
});

const ImmortalLiquidUnilateralExitPackageSchema = Schema.Struct({
  schema: Schema.NonEmptyString,
  genesis_hash: LowerHex64Schema,
  network_id: Schema.NonEmptyString,
  asset_id: Schema.NonEmptyString,
  funding_transaction_id: LowerHex64Schema,
  funding_output_index: UInt32Schema,
  funding_amount: DecimalStringSchema,
  funding_script_pubkey: LowerHexSchema,
  path: Schema.NonEmptyString,
  script: LowerHexSchema,
  control_block: LowerHexSchema,
  timelock: UInt32Schema,
  spend_input_index: UInt32Schema,
  fee_output_index: UInt32Schema,
  fee_amount: DecimalStringSchema,
  transaction_sha256: LowerHex64Schema,
  transaction: LowerHexSchema,
  mode: Schema.Literals(["presigned", "wallet_sign"]),
  wallet_signing_handle_sha256: Schema.NullOr(LowerHex64Schema),
  preimage_recovery_ref: Schema.NullOr(Schema.NonEmptyString),
});

const ImmortalLiquidBeforeFundRequestSchema = Schema.Struct({
  swap_type: Schema.Literals(["submarine", "reverse", "chain"]),
  purpose: Schema.Literals(["requester_broadcast", "counterparty_lock"]),
  input_asset_id: Schema.NonEmptyString,
  output_asset_id: Schema.NonEmptyString,
  funding: ImmortalLiquidFundingVerificationInputSchema,
  exit_package: ImmortalLiquidUnilateralExitPackageSchema,
});

const ImmortalLiquidRecoveryFundingSchema = Schema.Struct({
  transaction_id: LowerHex64Schema,
  transaction_template: LowerHexSchema,
  transaction_template_sha256: LowerHex64Schema,
  output_index: UInt32Schema,
  amount: DecimalStringSchema,
  script_pubkey: LowerHexSchema,
  confirmation_policy_sha256: LowerHex64Schema,
});

const ImmortalLiquidRecoveryExitSchema = Schema.Struct({
  mode: Schema.NonEmptyString,
  path: Schema.NonEmptyString,
  transaction_template_sha256: LowerHex64Schema,
  transaction_template: LowerHexSchema,
  signed_transaction: Schema.NullOr(LowerHexSchema),
  signer_ref: Schema.NullOr(Schema.NonEmptyString),
  transaction_version: Schema.Number.check(
    Schema.isInt(),
    Schema.isGreaterThanOrEqualTo(-2_147_483_648),
    Schema.isLessThanOrEqualTo(2_147_483_647),
  ),
  lock_time: UInt32Schema,
  input_sequence: UInt32Schema,
  input_index: UInt32Schema,
  signature_hash: LowerHex64Schema,
  sighash_type: Schema.NonEmptyString,
  destination_script_pubkey: LowerHexSchema,
  earliest_broadcast_height: DecimalStringSchema,
  latest_safe_broadcast_height: DecimalStringSchema,
  fee_policy: Schema.Struct({
    target_blocks: NonNegativeSafeIntegerSchema,
    maximum_fee: DecimalStringSchema,
    bump_mode: Schema.NonEmptyString,
  }),
});

const ImmortalLiquidRecoveryVerificationSchema = Schema.Struct({
  quote_id: LowerHex64Schema,
  verifier_digest: LowerHex64Schema,
  swap_tree_sha256: LowerHex64Schema,
  genesis_hash: LowerHex64Schema,
  taproot_script: LowerHexSchema,
  taproot_control_block: LowerHexSchema,
  taproot_tree: Schema.optionalKey(Schema.Json),
  fee_output_index: UInt32Schema,
  fee_amount: DecimalStringSchema,
});

const ImmortalLiquidRecoveryPackageSchema = Schema.Struct({
  schema: Schema.NonEmptyString,
  profile: Schema.NonEmptyString,
  profile_version: NonNegativeSafeIntegerSchema,
  order_id: LowerHex64Schema,
  swap_contract_ids: Schema.Array(LowerHex64Schema),
  contract_sha256: LowerHex64Schema,
  participant_role: Schema.Literals(["requester", "provider"]),
  leg_id: Schema.NonEmptyString,
  network_id: Schema.NonEmptyString,
  asset_id: Schema.NonEmptyString,
  effect_id: LowerHex64Schema,
  funding: ImmortalLiquidRecoveryFundingSchema,
  exit: ImmortalLiquidRecoveryExitSchema,
  verification: ImmortalLiquidRecoveryVerificationSchema,
  secret_commitments: Schema.Struct({
    payment_hash: LowerHex64Schema,
    preimage_recovery_ref: Schema.NullOr(Schema.NonEmptyString),
  }),
  broadcast: Schema.Struct({
    mode: Schema.NonEmptyString,
    rpc_method: Schema.NonEmptyString,
    network_id: Schema.NonEmptyString,
    genesis_hash: LowerHex64Schema,
  }),
});

const ImmortalLiquidFundingBindingSchema = Schema.Struct({
  contract_sha256: LowerHex64Schema,
  contract_ids: Schema.Array(LowerHex64Schema),
  leg_id: Schema.NonEmptyString,
  exit_effect_id: LowerHex64Schema,
  exit_package_sha256: LowerHex64Schema,
  request: ImmortalLiquidBeforeFundRequestSchema,
  transaction_id: LowerHex64Schema,
  output_index: UInt32Schema,
  amount: DecimalStringSchema,
  exit_transaction_sha256: LowerHex64Schema,
  recovery_package: ImmortalLiquidRecoveryPackageSchema,
  provenance: Schema.Struct({
    authority: Schema.Literal("local_elementsd"),
    network_id: Schema.NonEmptyString,
    genesis_hash: LowerHex64Schema,
    pegged_asset: Schema.NonEmptyString,
    funding_transaction_sha256: LowerHex64Schema,
    output_index: UInt32Schema,
    confidentiality: Schema.Literals(["explicit", "confidential"]),
    unblinded_transaction_sha256: Schema.NullOr(LowerHex64Schema),
  }),
});

export const ImmortalFundingActionSchema = Schema.Union(
  [
    Schema.Struct({
      action: Schema.Literal("broadcast_bitcoin"),
      effect_id: LowerHex64Schema,
      leg_id: Schema.NonEmptyString,
      raw_transaction: LowerHexSchema,
    }),
    Schema.Struct({
      action: Schema.Literal("broadcast_liquid"),
      effect_id: LowerHex64Schema,
      leg_id: Schema.NonEmptyString,
      raw_transaction: LowerHexSchema,
      transaction_id: LowerHex64Schema,
      output_index: UInt32Schema,
      exit_package_sha256: LowerHex64Schema,
    }),
    Schema.Struct({
      action: Schema.Literal("pay_lightning_invoice"),
      effect_id: LowerHex64Schema,
      leg_id: Schema.NonEmptyString,
      invoice: Schema.NonEmptyString,
      maximum_routing_fee: DecimalStringSchema,
      invoice_expires_at: NonNegativeSafeIntegerSchema,
      minimum_final_cltv_delta: NonNegativeSafeIntegerSchema,
      hold_invoice_required: Schema.Boolean,
      hold_expiry_height: UInt32Schema,
    }),
  ],
  { mode: "oneOf" },
);
export type ImmortalFundingAction = typeof ImmortalFundingActionSchema.Type;

export const ImmortalFundingRequestSchema = Schema.Struct({
  session_id: LowerHex64Schema,
  order_id: LowerHex64Schema,
  quote_id: LowerHex64Schema,
  swap_type: Schema.Literals(["submarine", "reverse", "chain"]),
  liquid: Schema.optionalKey(ImmortalLiquidFundingBindingSchema),
  action: ImmortalFundingActionSchema,
});
export type ImmortalFundingRequest = typeof ImmortalFundingRequestSchema.Type;

export const ImmortalFundingResultSchema = Schema.Struct({
  funding_request: ImmortalFundingRequestSchema,
  snapshot_json_hex: Schema.String.check(Schema.isPattern(/^(?:[0-9a-f]{2})+$/)),
});
export type ImmortalFundingResult = typeof ImmortalFundingResultSchema.Type;

export const ImmortalContractDraftSchema = Schema.Record(Schema.String, Schema.Json);
export type ImmortalContractDraft = typeof ImmortalContractDraftSchema.Type;

export const ImmortalExitPackageInspectionSchema = Schema.Struct({
  document: Schema.Json,
  commitment_sha256: LowerHex64Schema,
  effect_id: LowerHex64Schema,
  path: Schema.NonEmptyString,
  mode: Schema.NonEmptyString,
  unsigned_transaction_hex: Schema.NullOr(LowerHexSchema),
  signing_digest: Schema.NullOr(LowerHex64Schema),
});
export type ImmortalExitPackageInspection = typeof ImmortalExitPackageInspectionSchema.Type;

const ImmortalBrowserResponseSchema = Schema.Struct({
  schema: Schema.Literal(IMMORTAL_BROWSER_ABI_SCHEMA),
  abi_version: Schema.Literal(IMMORTAL_BROWSER_ABI_VERSION),
  source_revision: Schema.Literal(IMMORTAL_BROWSER_SOURCE_REVISION),
  requester_api_sha256: Schema.Literal(IMMORTAL_REQUESTER_API_SHA256),
  result: Schema.optionalKey(Schema.Json),
  error: Schema.optionalKey(Schema.Struct({ code: Schema.NonEmptyString, detail: Schema.String })),
});

export class ImmortalBrowserAbiError extends Schema.TaggedErrorClass<ImmortalBrowserAbiError>()(
  "MktSwp.ImmortalBrowserAbiError",
  {
    stage: Schema.Literals([
      "fetch",
      "compile",
      "instantiate",
      "compatibility",
      "request",
      "invoke",
      "response",
    ]),
    code: Schema.NonEmptyString,
    detail: Schema.String,
  },
) {}

type NumericExport = (...arguments_: readonly number[]) => number;

interface BrowserAbiExports {
  readonly abiVersion: NumericExport;
  readonly maximumRequestBytes: NumericExport;
  readonly maximumResponseBytes: NumericExport;
  readonly requestReset: NumericExport;
  readonly requestPush: NumericExport;
  readonly invoke: NumericExport;
  readonly responseLength: NumericExport;
  readonly responseByte: NumericExport;
}

type InvokeLock = ReturnType<typeof Semaphore.makeUnsafe>;
const invokeLocks = new WeakMap<WebAssembly.Exports, InvokeLock>();

const invokeLockFor = (exports: WebAssembly.Exports): InvokeLock => {
  const existing = invokeLocks.get(exports);
  if (existing !== undefined) return existing;
  const created = Semaphore.makeUnsafe(1);
  invokeLocks.set(exports, created);
  return created;
};

export type ImmortalWasmSource = Response | ArrayBuffer | ArrayBufferView | URL | string;

const strictDecode = <SchemaType extends Schema.ConstraintDecoder<unknown, never>>(
  schema: SchemaType,
  input: unknown,
  stage: ImmortalBrowserAbiError["stage"],
  code: string,
): Effect.Effect<SchemaType["Type"], ImmortalBrowserAbiError> =>
  Schema.decodeUnknownEffect(schema)(input, {
    onExcessProperty: "error",
  }).pipe(
    Effect.mapError(
      (cause) =>
        new ImmortalBrowserAbiError({
          stage,
          code,
          detail: String(cause),
        }),
    ),
  );

const abiError = (
  stage: ImmortalBrowserAbiError["stage"],
  code: string,
  cause: unknown,
): ImmortalBrowserAbiError =>
  new ImmortalBrowserAbiError({
    stage,
    code,
    detail: cause instanceof Error ? cause.message : String(cause),
  });

const sourceBytes = (
  source: ImmortalWasmSource,
): Effect.Effect<ArrayBuffer, ImmortalBrowserAbiError> => {
  if (typeof Response !== "undefined" && source instanceof Response) {
    if (!source.ok) {
      return Effect.fail(
        abiError(
          "fetch",
          "browser_wasm_fetch_failed",
          `requester engine fetch returned HTTP ${source.status}`,
        ),
      );
    }
    return Effect.tryPromise({
      try: () => source.arrayBuffer(),
      catch: (cause) => abiError("fetch", "browser_wasm_fetch_failed", cause),
    });
  }
  if (source instanceof ArrayBuffer) return Effect.succeed(source);
  if (ArrayBuffer.isView(source)) {
    return Effect.succeed(
      Uint8Array.from(new Uint8Array(source.buffer, source.byteOffset, source.byteLength)).buffer,
    );
  }
  if (typeof source === "string" || source instanceof URL) {
    return Effect.tryPromise({
      try: async () => {
        const response = await fetch(source);
        if (!response.ok) {
          throw new Error(`requester engine fetch returned HTTP ${response.status}`);
        }
        return response.arrayBuffer();
      },
      catch: (cause) => abiError("fetch", "browser_wasm_fetch_failed", cause),
    });
  }
  return Effect.fail(abiError("fetch", "browser_wasm_source_invalid", "unsupported WASM source"));
};

const requiredFunction = (
  exports: WebAssembly.Exports,
  name: string,
): Effect.Effect<NumericExport, ImmortalBrowserAbiError> => {
  const candidate = exports[name];
  return typeof candidate === "function"
    ? Effect.succeed((...arguments_: readonly number[]) => {
        const result: unknown = candidate(...arguments_);
        return typeof result === "number" ? result : Number.NaN;
      })
    : Effect.fail(
        abiError(
          "compatibility",
          "browser_wasm_export_missing",
          `the requester engine omits ${name}`,
        ),
      );
};

const bindExports = (
  exports: WebAssembly.Exports,
): Effect.Effect<BrowserAbiExports, ImmortalBrowserAbiError> =>
  Effect.all({
    abiVersion: requiredFunction(exports, "immortal_mkt_swp_browser_abi_version"),
    maximumRequestBytes: requiredFunction(exports, "immortal_mkt_swp_browser_max_request_bytes"),
    maximumResponseBytes: requiredFunction(exports, "immortal_mkt_swp_browser_max_response_bytes"),
    requestReset: requiredFunction(exports, "immortal_mkt_swp_browser_request_reset"),
    requestPush: requiredFunction(exports, "immortal_mkt_swp_browser_request_push"),
    invoke: requiredFunction(exports, "immortal_mkt_swp_browser_invoke"),
    responseLength: requiredFunction(exports, "immortal_mkt_swp_browser_response_len"),
    responseByte: requiredFunction(exports, "immortal_mkt_swp_browser_response_byte"),
  });

const readCompatibilityValue = (
  read: NumericExport,
  label: string,
): Effect.Effect<number, ImmortalBrowserAbiError> =>
  Effect.try({
    try: read,
    catch: (cause) =>
      abiError(
        "compatibility",
        "browser_wasm_export_failed",
        `${label} failed: ${cause instanceof Error ? cause.message : String(cause)}`,
      ),
  });

const exchange = (
  exports: BrowserAbiExports,
  request: Uint8Array,
): Effect.Effect<Uint8Array, ImmortalBrowserAbiError> =>
  Effect.try({
    try: () => {
      const checkStatus = (status: number, action: string): void => {
        if (status !== 0) {
          throw abiError(
            "invoke",
            "browser_wasm_state_error",
            `the requester engine failed during ${action} with status ${status}`,
          );
        }
      };
      checkStatus(exports.requestReset(), "reset");
      for (const byte of request) {
        checkStatus(exports.requestPush(byte), "request transfer");
      }
      checkStatus(exports.invoke(), "invoke");

      const length = exports.responseLength();
      if (
        !Number.isSafeInteger(length) ||
        length <= 0 ||
        length > IMMORTAL_BROWSER_MAX_RESPONSE_BYTES
      ) {
        throw abiError(
          "response",
          "browser_response_bound",
          "the requester engine returned an invalid response length",
        );
      }
      const response = new Uint8Array(length);
      for (let index = 0; index < length; index += 1) {
        const byte = exports.responseByte(index);
        if (!Number.isSafeInteger(byte) || byte < 0 || byte > 255) {
          throw abiError(
            "response",
            "browser_response_invalid",
            "the requester engine response ended early",
          );
        }
        response[index] = byte;
      }
      return response;
    },
    catch: (cause) =>
      cause instanceof ImmortalBrowserAbiError
        ? cause
        : abiError("invoke", "browser_wasm_trapped", cause),
  });

const invokeOperation: unique symbol = Symbol("ImmortalBrowserClient.invokeOperation");

export interface ImmortalBrowserClient {
  readonly metadata: ImmortalBrowserMetadata;
  readonly [invokeOperation]: (
    operation: ImmortalBrowserOperation,
    input: Schema.Json,
  ) => Effect.Effect<Schema.Json, ImmortalBrowserAbiError>;
}

export class ImmortalBrowserEngine extends Context.Service<
  ImmortalBrowserEngine,
  ImmortalBrowserClient
>()("@openagentsinc/mkt-swp/ImmortalBrowserEngine") {}

const makeClient = (
  exports: BrowserAbiExports,
  invokeLock: InvokeLock,
): Effect.Effect<ImmortalBrowserClient, ImmortalBrowserAbiError> =>
  Effect.gen(function* () {
    const abiVersion = yield* readCompatibilityValue(
      exports.abiVersion,
      "browser ABI version export",
    );
    if (abiVersion !== IMMORTAL_BROWSER_ABI_VERSION) {
      return yield* abiError(
        "compatibility",
        "browser_abi_version_mismatch",
        `expected browser ABI ${IMMORTAL_BROWSER_ABI_VERSION}`,
      );
    }
    const maximumRequestBytes = yield* readCompatibilityValue(
      exports.maximumRequestBytes,
      "maximum request bytes export",
    );
    const maximumResponseBytes = yield* readCompatibilityValue(
      exports.maximumResponseBytes,
      "maximum response bytes export",
    );
    if (
      maximumRequestBytes !== IMMORTAL_BROWSER_MAX_REQUEST_BYTES ||
      maximumResponseBytes !== IMMORTAL_BROWSER_MAX_RESPONSE_BYTES
    ) {
      return yield* abiError(
        "compatibility",
        "browser_abi_bounds_mismatch",
        "requester engine bounds do not match the pinned browser contract",
      );
    }

    const invokeUnlocked = Effect.fn("MktSwp.ImmortalBrowserClient.invokeUnlocked")(function* (
      operation: ImmortalBrowserOperation,
      input: Schema.Json,
    ) {
      const validatedInput = yield* strictDecode(
        Schema.Json,
        input,
        "request",
        "browser_input_invalid",
      );
      const request = new TextEncoder().encode(
        JSON.stringify({
          abi_version: IMMORTAL_BROWSER_ABI_VERSION,
          operation,
          input: validatedInput,
        }),
      );
      if (request.byteLength > IMMORTAL_BROWSER_MAX_REQUEST_BYTES) {
        return yield* abiError(
          "request",
          "browser_request_bound",
          `request exceeds ${IMMORTAL_BROWSER_MAX_REQUEST_BYTES} bytes`,
        );
      }

      const response = yield* exchange(exports, request);
      const parsed = yield* Effect.try({
        try: () =>
          parseJsonRejectingDuplicateMembers(
            new TextDecoder("utf-8", { fatal: true }).decode(response),
          ),
        catch: (cause) => abiError("response", "browser_response_invalid", cause),
      });
      const document = yield* strictDecode(
        ImmortalBrowserResponseSchema,
        parsed,
        "response",
        "browser_response_invalid",
      );
      if ((document.result === undefined) === (document.error === undefined)) {
        return yield* abiError(
          "response",
          "browser_response_invalid",
          "browser response must contain exactly one of result or error",
        );
      }
      if (document.error !== undefined) {
        return yield* new ImmortalBrowserAbiError({
          stage: "invoke",
          code: document.error.code,
          detail: document.error.detail,
        });
      }
      if (document.result === undefined) {
        return yield* abiError(
          "response",
          "browser_response_invalid",
          "browser response result is absent",
        );
      }
      return document.result;
    });
    const invoke = Effect.fn("MktSwp.ImmortalBrowserClient.invoke")(function* (
      operation: ImmortalBrowserOperation,
      input: Schema.Json,
    ) {
      return yield* invokeLock.withPermit(invokeUnlocked(operation, input));
    });

    const metadata = yield* invoke("metadata", {}).pipe(
      Effect.flatMap((result) =>
        strictDecode(
          ImmortalBrowserMetadataSchema,
          result,
          "compatibility",
          "browser_metadata_invalid",
        ),
      ),
    );
    if (
      metadata.operations.length !== IMMORTAL_BROWSER_OPERATIONS.length ||
      metadata.operations.some(
        (operation, index) => operation !== IMMORTAL_BROWSER_OPERATIONS[index],
      )
    ) {
      return yield* abiError(
        "compatibility",
        "browser_operations_mismatch",
        "requester engine operation inventory does not match the pinned contract",
      );
    }
    return { metadata, [invokeOperation]: invoke };
  });

export const bindImmortalBrowserClient = Effect.fn("MktSwp.bindImmortalBrowserClient")(function* (
  exports: WebAssembly.Exports,
) {
  const invokeLock = invokeLockFor(exports);
  const bound = yield* bindExports(exports);
  return yield* makeClient(bound, invokeLock);
});

export const loadImmortalBrowserClient = Effect.fn("MktSwp.loadImmortalBrowserClient")(function* (
  source: ImmortalWasmSource,
) {
  const bytes = yield* sourceBytes(source);
  const module = yield* Effect.tryPromise({
    try: () => WebAssembly.compile(bytes),
    catch: (cause) => abiError("compile", "browser_wasm_compile_failed", cause),
  });
  if (WebAssembly.Module.imports(module).length !== 0) {
    return yield* abiError(
      "compatibility",
      "browser_wasm_imports_forbidden",
      "the requester engine must not import host authority",
    );
  }
  const instance = yield* Effect.tryPromise({
    try: () => WebAssembly.instantiate(module, {}),
    catch: (cause) => abiError("instantiate", "browser_wasm_instantiate_failed", cause),
  });
  return yield* bindImmortalBrowserClient(instance.exports);
});

export const immortalBrowserEngineLayer = (
  source: ImmortalWasmSource,
): Layer.Layer<ImmortalBrowserEngine, ImmortalBrowserAbiError> =>
  Layer.effect(
    ImmortalBrowserEngine,
    loadImmortalBrowserClient(source).pipe(
      Effect.map((client) => ImmortalBrowserEngine.of(client)),
    ),
  );

const invokeDecoded = <SchemaType extends Schema.ConstraintDecoder<unknown, never>>(
  client: ImmortalBrowserClient,
  operation: ImmortalBrowserOperation,
  input: Schema.Json,
  outputSchema: SchemaType,
): Effect.Effect<SchemaType["Type"], ImmortalBrowserAbiError> =>
  client[invokeOperation](operation, input).pipe(
    Effect.flatMap((result) =>
      strictDecode(outputSchema, result, "response", `browser_${operation}_response_invalid`),
    ),
  );

export const requesterRfq = (
  client: ImmortalBrowserClient,
  input: Schema.Json,
): Effect.Effect<ImmortalSigningRequest, ImmortalBrowserAbiError> =>
  invokeDecoded(client, "requester_rfq", input, ImmortalSigningRequestSchema);

export const validateImmortalOffering = (
  client: ImmortalBrowserClient,
  input: Schema.Json,
): Effect.Effect<ImmortalNostrEvent, ImmortalBrowserAbiError> =>
  invokeDecoded(client, "validate_offering", input, ImmortalNostrEventSchema);

export const validateImmortalDelivery = (
  client: ImmortalBrowserClient,
  input: Schema.Json,
): Effect.Effect<ImmortalSignedRecordDelivery, ImmortalBrowserAbiError> =>
  invokeDecoded(client, "validate_delivery", input, ImmortalSignedRecordDeliverySchema);

export const requesterOrder = (
  client: ImmortalBrowserClient,
  input: Schema.Json,
): Effect.Effect<ImmortalSigningRequest, ImmortalBrowserAbiError> =>
  invokeDecoded(client, "requester_order", input, ImmortalSigningRequestSchema);

export const requesterContract = (
  client: ImmortalBrowserClient,
  input: Schema.Json,
): Effect.Effect<ImmortalSigningRequest, ImmortalBrowserAbiError> =>
  invokeDecoded(client, "requester_contract", input, ImmortalSigningRequestSchema);

export const requesterContractDraft = (
  client: ImmortalBrowserClient,
  input: Schema.Json,
): Effect.Effect<ImmortalContractDraft, ImmortalBrowserAbiError> =>
  invokeDecoded(client, "requester_contract_draft", input, ImmortalContractDraftSchema);

export const requesterCancel = (
  client: ImmortalBrowserClient,
  input: Schema.Json,
): Effect.Effect<ImmortalSigningRequest, ImmortalBrowserAbiError> =>
  invokeDecoded(client, "requester_cancel", input, ImmortalSigningRequestSchema);

export const requesterClose = (
  client: ImmortalBrowserClient,
  input: Schema.Json,
): Effect.Effect<ImmortalSigningRequest, ImmortalBrowserAbiError> =>
  invokeDecoded(client, "requester_close", input, ImmortalSigningRequestSchema);

export const verifySignedRequesterRecord = (
  client: ImmortalBrowserClient,
  input: Schema.Json,
): Effect.Effect<ImmortalNostrEvent, ImmortalBrowserAbiError> =>
  invokeDecoded(client, "verify_signed", input, ImmortalNostrEventSchema);

export const createRequesterSession = (
  client: ImmortalBrowserClient,
  input: Schema.Json,
): Effect.Effect<ImmortalSessionResult, ImmortalBrowserAbiError> =>
  invokeDecoded(client, "session_create", input, ImmortalSessionResultSchema);

export const ingestRequesterSession = (
  client: ImmortalBrowserClient,
  input: Schema.Json,
): Effect.Effect<ImmortalSessionIngestResult, ImmortalBrowserAbiError> =>
  invokeDecoded(client, "session_ingest", input, ImmortalSessionIngestResultSchema);

export const restoreRequesterSession = (
  client: ImmortalBrowserClient,
  input: Schema.Json,
): Effect.Effect<ImmortalSessionResult, ImmortalBrowserAbiError> =>
  invokeDecoded(client, "session_restore", input, ImmortalSessionResultSchema);

export const inspectImmortalExitPackage = (
  client: ImmortalBrowserClient,
  input: Schema.Json,
): Effect.Effect<ImmortalExitPackageInspection, ImmortalBrowserAbiError> =>
  invokeDecoded(client, "exit_package_inspect", input, ImmortalExitPackageInspectionSchema);

export const prepareFundingRequest = (
  client: ImmortalBrowserClient,
  input: Schema.Json,
): Effect.Effect<ImmortalFundingRequest, ImmortalBrowserAbiError> =>
  invokeDecoded(client, "prepare_funding_request", input, ImmortalFundingRequestSchema);

export const verifyBeforeFund = (
  client: ImmortalBrowserClient,
  input: Schema.Json,
): Effect.Effect<ImmortalFundingResult, ImmortalBrowserAbiError> =>
  invokeDecoded(client, "verify_before_fund", input, ImmortalFundingResultSchema);
