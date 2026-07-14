import { describe, expect, test } from "vite-plus/test"
import { openAgentsMcpAuthorityClasses } from "@openagentsinc/mcp-contract"

import {
  decodeEnvironmentCapabilityGrant,
  decodeEnvironmentTokenExchangeRequest,
  decodeEnvironmentTokenExchangeResponse,
  ENVIRONMENT_BOOTSTRAP_SCOPES,
  ENVIRONMENT_CAPABILITY_GRANT_SCHEMA_VERSION,
  ENVIRONMENT_CAPABILITY_SCOPES,
  ENVIRONMENT_CLIENT_DEFAULT_SCOPES,
  ENVIRONMENT_TOKEN_EXCHANGE_SCHEMA_VERSION,
  evaluateEnvironmentScopeExchange,
  OPENAGENTS_ACCESS_TOKEN_TYPE,
  OPENAGENTS_ENVIRONMENT_BOOTSTRAP_TOKEN_TYPE,
  OPENAGENTS_TOKEN_EXCHANGE_GRANT_TYPE,
  type EnvironmentCapabilityScope,
} from "./index.js"

const THUMBPRINT = "kzHF9VGCd25EXkFCXhFC1x5DRSK0LSSKfw8DzTFVGCd"

const exchangeRequest = (overrides: Record<string, unknown> = {}) => ({
  schema: ENVIRONMENT_TOKEN_EXCHANGE_SCHEMA_VERSION,
  grantType: OPENAGENTS_TOKEN_EXCHANGE_GRANT_TYPE,
  subjectToken: "bootstrap.one-time.credential.0001",
  subjectTokenType: OPENAGENTS_ENVIRONMENT_BOOTSTRAP_TOKEN_TYPE,
  requestedTokenType: OPENAGENTS_ACCESS_TOKEN_TYPE,
  scopes: ["operator_read", "coding_session_control"],
  clientKeyThumbprint: THUMBPRINT,
  clientLabel: "Owner iPhone",
  ...overrides,
})

const grantRecord = (overrides: Record<string, unknown> = {}) => ({
  schema: ENVIRONMENT_CAPABILITY_GRANT_SCHEMA_VERSION,
  grantRef: "grant.env.pylon-m4.client-phone",
  ownerRef: "owner.chris",
  environmentRef: "environment.pylon.m4-desktop",
  scopes: ["operator_read", "coding_session_control"],
  clientKeyThumbprint: THUMBPRINT,
  issuedAt: "2026-07-14T05:00:00.000Z",
  expiresAt: "2026-07-14T06:00:00.000Z",
  state: "active",
  tokenMaterial: "excluded",
  ...overrides,
})

describe("environment capability scope vocabulary", () => {
  test("every environment scope is an existing OpenAgents MCP authority class", () => {
    for (const scope of ENVIRONMENT_CAPABILITY_SCOPES) {
      expect(openAgentsMcpAuthorityClasses).toContain(scope)
    }
  })

  test("ordinary client scopes exclude admin; bootstrap scopes include it", () => {
    expect(ENVIRONMENT_CLIENT_DEFAULT_SCOPES).not.toContain("admin")
    expect(ENVIRONMENT_BOOTSTRAP_SCOPES).toContain("admin")
    for (const scope of ENVIRONMENT_CLIENT_DEFAULT_SCOPES) {
      expect(ENVIRONMENT_BOOTSTRAP_SCOPES).toContain(scope)
    }
  })
})

describe("token exchange and grant contracts", () => {
  test("decodes an RFC 8693-shaped exchange request and response", () => {
    const request = decodeEnvironmentTokenExchangeRequest(exchangeRequest())
    expect(request.grantType).toBe("urn:ietf:params:oauth:grant-type:token-exchange")
    expect(request.clientKeyThumbprint).toBe(THUMBPRINT)
    const response = decodeEnvironmentTokenExchangeResponse({
      schema: ENVIRONMENT_TOKEN_EXCHANGE_SCHEMA_VERSION,
      accessToken: "issued.opaque.access.token.0001",
      issuedTokenType: OPENAGENTS_ACCESS_TOKEN_TYPE,
      tokenType: "DPoP",
      expiresInSeconds: 3600,
      scopes: ["operator_read"],
      clientKeyThumbprint: THUMBPRINT,
      grantRef: "grant.env.pylon-m4.client-phone",
    })
    expect(response.tokenType).toBe("DPoP")
  })

  test("rejects bearer-shaped responses and unbound exchange requests", () => {
    expect(() =>
      decodeEnvironmentTokenExchangeResponse({
        schema: ENVIRONMENT_TOKEN_EXCHANGE_SCHEMA_VERSION,
        accessToken: "issued.opaque.access.token.0001",
        issuedTokenType: OPENAGENTS_ACCESS_TOKEN_TYPE,
        tokenType: "Bearer",
        expiresInSeconds: 3600,
        scopes: ["operator_read"],
        clientKeyThumbprint: THUMBPRINT,
        grantRef: "grant.env.pylon-m4.client-phone",
      }),
    ).toThrow()
    expect(() =>
      decodeEnvironmentTokenExchangeRequest(exchangeRequest({ clientKeyThumbprint: undefined })),
    ).toThrow()
    expect(() =>
      decodeEnvironmentTokenExchangeRequest(exchangeRequest({ clientKeyThumbprint: "short" })),
    ).toThrow()
    expect(() =>
      decodeEnvironmentTokenExchangeRequest(exchangeRequest({ scopes: ["terminal:operate"] })),
    ).toThrow()
  })

  test("decodes a grant record bound to an ExecutionEnvironment identity and excludes material", () => {
    const grant = decodeEnvironmentCapabilityGrant(grantRecord())
    expect(grant.environmentRef).toBe("environment.pylon.m4-desktop")
    expect(grant.tokenMaterial).toBe("excluded")
    expect(() => decodeEnvironmentCapabilityGrant(grantRecord({ tokenMaterial: "inline" }))).toThrow()
    expect(() => decodeEnvironmentCapabilityGrant(grantRecord({ scopes: [] }))).toThrow()
    expect(() => decodeEnvironmentCapabilityGrant(grantRecord({ clientKeyThumbprint: "nope" }))).toThrow()
    expect(() => decodeEnvironmentCapabilityGrant(grantRecord({ environmentRef: "" }))).toThrow()
  })
})

describe("scope exchange is provably narrowing-only", () => {
  test("a strict subset request is granted exactly as requested", () => {
    const decision = evaluateEnvironmentScopeExchange({
      subjectScopes: [...ENVIRONMENT_BOOTSTRAP_SCOPES],
      requestedScopes: ["coding_session_control", "operator_read"],
    })
    expect(decision).toEqual({
      ok: true,
      grantedScopes: ["coding_session_control", "operator_read"],
    })
  })

  test("an empty request inherits the subject scopes (RFC 8693 default)", () => {
    const decision = evaluateEnvironmentScopeExchange({
      subjectScopes: ["workspace_read", "operator_read", "workspace_read"],
      requestedScopes: [],
    })
    expect(decision).toEqual({ ok: true, grantedScopes: ["operator_read", "workspace_read"] })
  })

  test("adversarial: requesting any scope beyond the subject grant rejects the whole exchange", () => {
    const decision = evaluateEnvironmentScopeExchange({
      subjectScopes: [...ENVIRONMENT_CLIENT_DEFAULT_SCOPES],
      requestedScopes: ["operator_read", "admin"],
    })
    expect(decision).toEqual({
      ok: false,
      reason: "scope_widening_rejected",
      offendingScopes: ["admin"],
    })
  })

  test("adversarial: an empty subject grant can never mint scopes", () => {
    expect(evaluateEnvironmentScopeExchange({ subjectScopes: [], requestedScopes: [] }))
      .toEqual({ ok: false, reason: "subject_scopes_empty", offendingScopes: [] })
    expect(evaluateEnvironmentScopeExchange({ subjectScopes: [], requestedScopes: ["operator_read"] }))
      .toEqual({ ok: false, reason: "subject_scopes_empty", offendingScopes: [] })
  })

  test("exhaustive over the full scope lattice: granted scopes are always a subset of subject scopes", () => {
    const vocabulary = ENVIRONMENT_CAPABILITY_SCOPES
    const subsets: EnvironmentCapabilityScope[][] = []
    for (let mask = 0; mask < 1 << vocabulary.length; mask += 1) {
      subsets.push(vocabulary.filter((_, index) => (mask & (1 << index)) !== 0))
    }
    let evaluated = 0
    for (const subjectScopes of subsets) {
      const subjectSet = new Set(subjectScopes)
      for (const requestedScopes of subsets) {
        const decision = evaluateEnvironmentScopeExchange({ subjectScopes, requestedScopes })
        evaluated += 1
        if (decision.ok) {
          // Narrowing-only: nothing outside the subject grant is ever issued.
          expect(decision.grantedScopes.every((scope) => subjectSet.has(scope))).toBe(true)
          // And nothing outside an explicit request is ever issued.
          if (requestedScopes.length > 0) {
            const requestedSet = new Set(requestedScopes)
            expect(decision.grantedScopes.every((scope) => requestedSet.has(scope))).toBe(true)
          }
        } else {
          const isWidening = requestedScopes.some((scope) => !subjectSet.has(scope))
          expect(decision.reason).toBe(subjectScopes.length === 0 ? "subject_scopes_empty" : "scope_widening_rejected")
          if (subjectScopes.length > 0) expect(isWidening).toBe(true)
        }
      }
    }
    expect(evaluated).toBe((1 << vocabulary.length) ** 2)
  })
})
