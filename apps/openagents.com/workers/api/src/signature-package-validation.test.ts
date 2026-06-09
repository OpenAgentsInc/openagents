import { Schema as S } from 'effect'
import { Effect } from 'effect'
import { describe, expect, test } from 'vitest'

import {
  SIGNATURE_PACKAGE_VALIDATION_MANIFEST_FIXTURE,
  SIGNATURE_PACKAGE_VALIDATION_REQUEST_FIXTURE,
  SignaturePackageValidationEndpoint,
  SignaturePackageValidationResult,
  SignaturePackageValidationUnsafe,
  validateSignaturePackage,
  signaturePackageProjectionHasPrivateMaterial,
} from './signature-package-validation'
import { handleSignaturePackageValidationApi } from './signature-package-validation-routes'
import {
  OPENAGENTS_UNSAFE_REDACTION_FIXTURES,
  openAgentsSerializedValueContainsUnsafeFixture,
} from './redaction-regression-fixtures'

const routeRequest = (
  body: unknown,
  method = 'POST',
): Request => {
  const init: RequestInit = {
    headers: {
      'content-type': 'application/json',
      'idempotency-key': 'validation_request.test.header_key',
    },
    method,
  }

  if (method === 'POST') {
    init.body = JSON.stringify(body)
  }

  return new Request(
    `https://openagents.com${SignaturePackageValidationEndpoint}`,
    init,
  )
}

const runRoute = async (
  body: unknown,
  method = 'POST',
): Promise<Response> =>
  Effect.runPromise(handleSignaturePackageValidationApi(routeRequest(body, method)))

describe('signature package validation', () => {
  test('validates complete manifests without granting install or promotion authority', () => {
    const result = validateSignaturePackage(
      SIGNATURE_PACKAGE_VALIDATION_REQUEST_FIXTURE,
      'agent',
    )

    expect(S.decodeUnknownSync(SignaturePackageValidationResult)(result))
      .toEqual(result)
    expect(result.status).toBe('valid')
    expect(result.blockerRefs).toEqual([])
    expect(result.installAllowed).toBe(false)
    expect(result.runtimePromotionAllowed).toBe(false)
    expect(result.publicMarketplaceListingAllowed).toBe(false)
    expect(result.deploymentAllowed).toBe(false)
    expect(result.paymentMutationAllowed).toBe(false)
    expect(result.manifest.sourceRefs).toEqual([])
    expect(result.createdAtDisplay).toBe('40 minutes ago')
    expect(result.updatedAtDisplay).toBe('35 minutes ago')
    expect(JSON.stringify(result)).not.toContain('2026-06-07T')
    expect(signaturePackageProjectionHasPrivateMaterial(result)).toBe(false)
  })

  test('returns deterministic invalid results for missing validation inputs', () => {
    const result = validateSignaturePackage({
      ...SIGNATURE_PACKAGE_VALIDATION_REQUEST_FIXTURE,
      manifest: {
        ...SIGNATURE_PACKAGE_VALIDATION_MANIFEST_FIXTURE,
        evidenceRequirementRefs: [],
        fixtureRefs: [],
        jsonRenderBindingRefs: [],
        receiptRequirementRefs: [],
        schemaRefs: [],
        selectorMetadataRefs: [],
      },
    })

    expect(result.status).toBe('invalid')
    expect(result.blockerRefs).toEqual([
      'blocker.signature_package.schema_refs_missing',
      'blocker.signature_package.fixture_refs_missing',
      'blocker.signature_package.evidence_requirements_missing',
      'blocker.signature_package.receipt_requirements_missing',
      'blocker.signature_package.selector_metadata_missing',
      'blocker.signature_package.json_render_bindings_missing',
    ])
    expect(result.validationResultRef).toBe(
      'validation_result.validation_request.signature.example_site_builder.package.signature.example_site_builder.version.signature.example_site_builder.v1',
    )
  })

  test('rejects private package source, raw prompts, provider payloads, secrets, and timestamps', () => {
    for (const fixture of [
      ...OPENAGENTS_UNSAFE_REDACTION_FIXTURES,
      { label: 'private package source', value: 'package_source_private.repo' },
      { label: 'raw prompt', value: 'raw_prompt.system' },
      { label: 'provider payload', value: 'provider_payload.raw' },
      { label: 'raw schema', value: 'raw_schema.private' },
    ]) {
      expect(() =>
        validateSignaturePackage({
          ...SIGNATURE_PACKAGE_VALIDATION_REQUEST_FIXTURE,
          manifest: {
            ...SIGNATURE_PACKAGE_VALIDATION_MANIFEST_FIXTURE,
            sourceRefs: [fixture.value],
          },
        }),
      ).toThrow(SignaturePackageValidationUnsafe)
    }
  })

  test('serves the validation route as read-only no-store JSON', async () => {
    const response = await runRoute({
      manifest: SIGNATURE_PACKAGE_VALIDATION_MANIFEST_FIXTURE,
      nowIso: SIGNATURE_PACKAGE_VALIDATION_REQUEST_FIXTURE.nowIso,
    })
    const body = await response.json() as SignaturePackageValidationResult

    expect(response.status).toBe(200)
    expect(response.headers.get('cache-control')).toBe('no-store')
    expect(body.status).toBe('valid')
    expect(body.validationRequestRef).toBe(
      'validation_request.test.header_key',
    )
    expect(body.installAllowed).toBe(false)
    expect(body.runtimePromotionAllowed).toBe(false)
    expect(body.publicMarketplaceListingAllowed).toBe(false)
    expect(body.deploymentAllowed).toBe(false)
    expect(body.paymentMutationAllowed).toBe(false)
    expect(openAgentsSerializedValueContainsUnsafeFixture(body)).toBe(false)
  })

  test('returns validation blockers through the route without failing the request', async () => {
    const response = await runRoute({
      manifest: {
        ...SIGNATURE_PACKAGE_VALIDATION_MANIFEST_FIXTURE,
        fixtureRefs: [],
        schemaRefs: [],
      },
      nowIso: SIGNATURE_PACKAGE_VALIDATION_REQUEST_FIXTURE.nowIso,
      validationRequestRef: 'validation_request.route.invalid',
    })
    const body = await response.json() as SignaturePackageValidationResult

    expect(response.status).toBe(200)
    expect(body.status).toBe('invalid')
    expect(body.blockerRefs).toEqual([
      'blocker.signature_package.schema_refs_missing',
      'blocker.signature_package.fixture_refs_missing',
    ])
  })

  test('rejects invalid methods and unsafe route payloads', async () => {
    const methodResponse = await runRoute({}, 'GET')
    const unsafeResponse = await runRoute({
      manifest: {
        ...SIGNATURE_PACKAGE_VALIDATION_MANIFEST_FIXTURE,
        sourceRefs: ['raw_prompt.private'],
      },
    })
    const unsafeBody = await unsafeResponse.json() as { error: string }

    expect(methodResponse.status).toBe(405)
    expect(unsafeResponse.status).toBe(400)
    expect(unsafeBody.error).toBe('signature_package_validation_unsafe')
  })
})
