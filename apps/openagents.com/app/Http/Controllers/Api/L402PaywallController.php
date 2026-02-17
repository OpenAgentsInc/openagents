<?php

namespace App\Http\Controllers\Api;

use App\Exceptions\L402\ApertureReconcileException;
use App\Http\Controllers\Controller;
use App\Models\L402Paywall;
use App\OpenApi\Responses\DataObjectResponse;
use App\OpenApi\Responses\ForbiddenResponse;
use App\OpenApi\Responses\NotFoundResponse;
use App\OpenApi\Responses\UnauthorizedResponse;
use App\OpenApi\Responses\ValidationErrorResponse;
use App\Services\L402\L402PaywallOperatorService;
use App\Support\AdminAccess;
use Illuminate\Http\JsonResponse;
use Illuminate\Http\Request;
use Illuminate\Validation\ValidationException;
use Vyuldashev\LaravelOpenApi\Attributes as OpenApi;

#[OpenApi\PathItem]
class L402PaywallController extends Controller
{
    /**
     * Create a seller paywall route and reconcile Aperture configuration.
     */
    #[OpenApi\Operation(tags: ['L402'])]
    #[OpenApi\Response(factory: DataObjectResponse::class, statusCode: 201)]
    #[OpenApi\Response(factory: UnauthorizedResponse::class, statusCode: 401)]
    #[OpenApi\Response(factory: ForbiddenResponse::class, statusCode: 403)]
    #[OpenApi\Response(factory: ValidationErrorResponse::class, statusCode: 422)]
    public function store(Request $request, L402PaywallOperatorService $service): JsonResponse
    {
        $user = $this->assertOperator($request);

        $validated = $request->validate([
            'name' => ['required', 'string', 'max:120'],
            'hostRegexp' => ['required', 'string', 'max:255', function (string $attribute, mixed $value, \Closure $fail): void {
                $this->validateRegexBody($attribute, $value, $fail);
            }],
            'pathRegexp' => ['required', 'string', 'max:255', function (string $attribute, mixed $value, \Closure $fail): void {
                $this->validateRegexBody($attribute, $value, $fail, mustStartWithPathAnchor: true);
            }],
            'priceMsats' => ['required', 'integer', 'min:1', 'max:1000000000000'],
            'upstream' => ['required', 'string', 'url', 'max:2048', 'starts_with:http://,https://'],
            'enabled' => ['sometimes', 'boolean'],
            'metadata' => ['sometimes', 'array'],
        ]);

        try {
            $result = $service->create($user, $validated);
        } catch (ApertureReconcileException $exception) {
            return $this->reconcileFailureResponse($exception);
        }

        /** @var L402Paywall $paywall */
        $paywall = $result['paywall'];

        return response()->json([
            'data' => [
                'paywall' => $this->paywallPayload($paywall),
                'deployment' => $result['deployment'],
                'mutationEventId' => $result['mutationEventId'],
            ],
        ], 201);
    }

    /**
     * Update an existing seller paywall and reconcile Aperture configuration.
     */
    #[OpenApi\Operation(tags: ['L402'])]
    #[OpenApi\Response(factory: DataObjectResponse::class, statusCode: 200)]
    #[OpenApi\Response(factory: UnauthorizedResponse::class, statusCode: 401)]
    #[OpenApi\Response(factory: ForbiddenResponse::class, statusCode: 403)]
    #[OpenApi\Response(factory: NotFoundResponse::class, statusCode: 404)]
    #[OpenApi\Response(factory: ValidationErrorResponse::class, statusCode: 422)]
    public function update(string $paywallId, Request $request, L402PaywallOperatorService $service): JsonResponse
    {
        $user = $this->assertOperator($request);

        $validated = $request->validate([
            'name' => ['sometimes', 'required', 'string', 'max:120'],
            'hostRegexp' => ['sometimes', 'required', 'string', 'max:255', function (string $attribute, mixed $value, \Closure $fail): void {
                $this->validateRegexBody($attribute, $value, $fail);
            }],
            'pathRegexp' => ['sometimes', 'required', 'string', 'max:255', function (string $attribute, mixed $value, \Closure $fail): void {
                $this->validateRegexBody($attribute, $value, $fail, mustStartWithPathAnchor: true);
            }],
            'priceMsats' => ['sometimes', 'required', 'integer', 'min:1', 'max:1000000000000'],
            'upstream' => ['sometimes', 'required', 'string', 'url', 'max:2048', 'starts_with:http://,https://'],
            'enabled' => ['sometimes', 'boolean'],
            'metadata' => ['sometimes', 'array'],
        ]);

        if ($validated === []) {
            throw ValidationException::withMessages([
                'payload' => 'At least one mutable paywall field must be provided.',
            ]);
        }

        $paywall = L402Paywall::query()->where('id', $paywallId)->first();
        if (! $paywall) {
            abort(404);
        }

        try {
            $result = $service->update($user, $paywall, $validated);
        } catch (ApertureReconcileException $exception) {
            return $this->reconcileFailureResponse($exception);
        }

        /** @var L402Paywall $updated */
        $updated = $result['paywall'];

        return response()->json([
            'data' => [
                'paywall' => $this->paywallPayload($updated),
                'deployment' => $result['deployment'],
                'mutationEventId' => $result['mutationEventId'],
            ],
        ]);
    }

    /**
     * Delete (soft-delete) a seller paywall and reconcile Aperture configuration.
     */
    #[OpenApi\Operation(tags: ['L402'])]
    #[OpenApi\Response(factory: DataObjectResponse::class, statusCode: 200)]
    #[OpenApi\Response(factory: UnauthorizedResponse::class, statusCode: 401)]
    #[OpenApi\Response(factory: ForbiddenResponse::class, statusCode: 403)]
    #[OpenApi\Response(factory: NotFoundResponse::class, statusCode: 404)]
    #[OpenApi\Response(factory: ValidationErrorResponse::class, statusCode: 422)]
    public function destroy(string $paywallId, Request $request, L402PaywallOperatorService $service): JsonResponse
    {
        $user = $this->assertOperator($request);

        $paywall = L402Paywall::query()->where('id', $paywallId)->first();
        if (! $paywall) {
            abort(404);
        }

        try {
            $result = $service->delete($user, $paywall);
        } catch (ApertureReconcileException $exception) {
            return $this->reconcileFailureResponse($exception);
        }

        /** @var L402Paywall $deleted */
        $deleted = $result['paywall'];

        return response()->json([
            'data' => [
                'deleted' => true,
                'paywall' => $this->paywallPayload($deleted),
                'deployment' => $result['deployment'],
                'mutationEventId' => $result['mutationEventId'],
            ],
        ]);
    }

    /**
     * @param  \Closure(string): void  $fail
     */
    private function validateRegexBody(string $attribute, mixed $value, \Closure $fail, bool $mustStartWithPathAnchor = false): void
    {
        if (! is_string($value) || trim($value) === '') {
            $fail("The {$attribute} must be a non-empty regex body.");

            return;
        }

        if ($mustStartWithPathAnchor && ! str_starts_with($value, '^/')) {
            $fail("The {$attribute} must start with '^/' to scope path matching.");

            return;
        }

        $candidate = '/'.str_replace('/', '\\/', $value).'/';

        if (@preg_match($candidate, 'openagents') === false) {
            $fail("The {$attribute} must be a valid regex body.");
        }
    }

    /**
     * @return array<string, mixed>
     */
    private function paywallPayload(L402Paywall $paywall): array
    {
        return [
            'id' => (string) $paywall->id,
            'ownerUserId' => (int) $paywall->owner_user_id,
            'name' => (string) $paywall->name,
            'hostRegexp' => (string) $paywall->host_regexp,
            'pathRegexp' => (string) $paywall->path_regexp,
            'priceMsats' => (int) $paywall->price_msats,
            'upstream' => (string) $paywall->upstream,
            'enabled' => (bool) $paywall->enabled,
            'metadata' => is_array($paywall->meta) ? $paywall->meta : [],
            'lastReconcileStatus' => $paywall->last_reconcile_status,
            'lastReconcileError' => $paywall->last_reconcile_error,
            'lastReconciledAt' => $paywall->last_reconciled_at?->toISOString(),
            'createdAt' => $paywall->created_at?->toISOString(),
            'updatedAt' => $paywall->updated_at?->toISOString(),
            'deletedAt' => $paywall->deleted_at?->toISOString(),
        ];
    }

    private function reconcileFailureResponse(ApertureReconcileException $exception): JsonResponse
    {
        return response()->json([
            'message' => $exception->getMessage(),
            'errorCode' => 'l402_reconcile_failed',
            'reverted' => true,
            'context' => $exception->context(),
        ], 422);
    }

    private function assertOperator(Request $request): \App\Models\User
    {
        $user = $request->user();
        if (! $user) {
            abort(401);
        }

        if (! AdminAccess::isAdminEmail($user->email)) {
            abort(403);
        }

        return $user;
    }
}
