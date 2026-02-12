import { Effect } from "effect";
import { describe, expect, it } from "@effect/vitest";

import { validateCredentialRoleMap } from "../src/runtime/credentials.js";

describe("lightning-ops credential validation", () => {
  it.effect("fails closed when required role credential is missing", () =>
    Effect.gen(function* () {
      const attempted = yield* Effect.either(
        validateCredentialRoleMap({
          settlement_read: "valid_settlement_credential_12345",
          operator_admin: "valid_operator_credential_12345",
        }),
      );

      expect(attempted._tag).toBe("Left");
      if (attempted._tag === "Left") {
        expect(attempted.left._tag).toBe("CredentialValidationError");
        if (attempted.left._tag === "CredentialValidationError") {
          expect(attempted.left.code).toBe("missing_credential_role");
          expect(attempted.left.role).toBe("gateway_invoice");
          expect(attempted.left.field).toBe("OA_LIGHTNING_OPS_CRED_GATEWAY_INVOICE");
        }
      }
    }),
  );

  it.effect("fails with deterministic invalid_credential_role taxonomy for malformed secrets", () =>
    Effect.gen(function* () {
      const attempted = yield* Effect.either(
        validateCredentialRoleMap({
          gateway_invoice: "too-short",
          settlement_read: "valid_settlement_credential_12345",
          operator_admin: "valid_operator_credential_12345",
        }),
      );

      expect(attempted._tag).toBe("Left");
      if (attempted._tag === "Left") {
        expect(attempted.left._tag).toBe("CredentialValidationError");
        if (attempted.left._tag === "CredentialValidationError") {
          expect(attempted.left.code).toBe("invalid_credential_role");
          expect(attempted.left.role).toBe("gateway_invoice");
        }
      }
    }),
  );

  it.effect("accepts valid role credentials deterministically", () =>
    Effect.gen(function* () {
      const validated = yield* validateCredentialRoleMap({
        gateway_invoice: "valid_gateway_credential_12345",
        settlement_read: "valid_settlement_credential_12345",
        operator_admin: "valid_operator_credential_12345",
      });

      expect(validated.gateway_invoice).toBe("valid_gateway_credential_12345");
      expect(validated.settlement_read).toBe("valid_settlement_credential_12345");
      expect(validated.operator_admin).toBe("valid_operator_credential_12345");
    }),
  );
});
