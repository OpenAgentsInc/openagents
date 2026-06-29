import { Effect, Schema as S } from 'effect'

import {
  SignaturePackageManifest,
  SignaturePackageValidationEndpoint,
  SignaturePackageValidationRequest,
  SignaturePackageValidationUnsafe,
  validateSignaturePackage,
} from './signature-package-validation'
import {
  methodNotAllowed,
  noStoreJsonResponse,
} from './http/responses'
import {
  isRecord,
  nestedUnknown,
  optionalString,
  readJsonObject,
} from './json-boundary'

class SignaturePackageValidationRouteInvalidRequest extends S.TaggedErrorClass<SignaturePackageValidationRouteInvalidRequest>()(
  'SignaturePackageValidationRouteInvalidRequest',
  {
    reason: S.String,
  },
) {}

const routeError = (
  error: unknown,
): SignaturePackageValidationUnsafe | SignaturePackageValidationRouteInvalidRequest =>
  error instanceof SignaturePackageValidationUnsafe
    ? error
    : new SignaturePackageValidationRouteInvalidRequest({
      reason: 'Invalid signature package validation request.',
    })

const validationErrorResponse = (error: unknown) => {
  if (error instanceof SignaturePackageValidationUnsafe) {
    return noStoreJsonResponse(
      {
        error: 'signature_package_validation_unsafe',
        reason: error.reason,
      },
      { status: 400 },
    )
  }

  if (error instanceof SignaturePackageValidationRouteInvalidRequest) {
    return noStoreJsonResponse(
      {
        error: 'signature_package_validation_invalid_request',
        reason: error.reason,
      },
      { status: 400 },
    )
  }

  return noStoreJsonResponse(
    { error: 'signature_package_validation_invalid_request' },
    { status: 400 },
  )
}

const requestFromBody = (
  request: Request,
  body: Record<string, unknown>,
): SignaturePackageValidationRequest => {
  const manifestBody = nestedUnknown(body, ['manifest'])
  const manifestUnknown = isRecord(manifestBody) ? manifestBody : body
  const manifest = S.decodeUnknownSync(SignaturePackageManifest)(
    manifestUnknown,
  )
  const validationRequestRef =
    optionalString(body.validationRequestRef) ??
    optionalString(request.headers.get('idempotency-key')) ??
    `validation_request.${manifest.packageRef}.${manifest.versionRef}`
  const nowIso =
    optionalString(body.nowIso) ??
    optionalString(body.requestedAtIso) ??
    manifest.updatedAtIso

  return S.decodeUnknownSync(SignaturePackageValidationRequest)({
    manifest,
    nowIso,
    validationRequestRef,
  })
}

export const handleSignaturePackageValidationApi = (
  request: Request,
) =>
  request.method !== 'POST'
    ? Effect.succeed(methodNotAllowed(['POST']))
    : Effect.tryPromise({
        try: async () => readJsonObject(request),
        catch: routeError,
      }).pipe(
        Effect.flatMap(body =>
          Effect.try({
            try: () => validateSignaturePackage(
              requestFromBody(request, body),
            ),
            catch: routeError,
          })
        ),
        Effect.map(result => noStoreJsonResponse(result)),
        Effect.catch(error => Effect.succeed(validationErrorResponse(error))),
      )

export const SignaturePackageValidationApiPath =
  SignaturePackageValidationEndpoint
