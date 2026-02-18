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
use App\Services\PostHogService;
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
    public function store(Request $request, L402PaywallOperatorService $service, PostHogService $posthog): JsonResponse
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
            $posthog->capture($user->email, 'l402.paywall_create_reconcile_failed', [
                'name' => (string) $validated['name'],
                'hostRegexp' => (string) $validated['hostRegexp'],
                'pathRegexp' => (string) $validated['pathRegexp'],
                'priceMsats' => (int) $validated['priceMsats'],
                'error' => $exception->getMessage(),
            ]);
            return $this->reconcileFailureResponse($exception);
        }

        /** @var L402Paywall $paywall */
        $paywall = $result['paywall'];
        $posthog->capture($user->email, 'l402.paywall_created', [
            'paywallId' => (string) $paywall->id,
            'name' => (string) $paywall->name,
            'priceMsats' => (int) $paywall->price_msats,
            'enabled' => (bool) $paywall->enabled,
        ]);

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
    public function update(string $paywallId, Request $request, L402PaywallOperatorService $service, PostHogService $posthog): JsonResponse
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
            $posthog->capture($user->email, 'l402.paywall_update_reconcile_failed', [
                'paywallId' => $paywallId,
                'fieldCount' => count($validated),
                'error' => $exception->getMessage(),
            ]);
            return $this->reconcileFailureResponse($exception);
        }

        /** @var L402Paywall $updated */
        $updated = $result['paywall'];
        $posthog->capture($user->email, 'l402.paywall_updated', [
            'paywallId' => (string) $updated->id,
            'fieldCount' => count($validated),
            'enabled' => (bool) $updated->enabled,
            'priceMsats' => (int) $updated->price_msats,
        ]);

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
    public function destroy(string $paywallId, Request $request, L402PaywallOperatorService $service, PostHogService $posthog): JsonResponse
    {
        $user = $this->assertOperator($request);

        $paywall = L402Paywall::query()->where('id', $paywallId)->first();
        if (! $paywall) {
            abort(404);
        }

        try {
            $result = $service->delete($user, $paywall);
        } catch (ApertureReconcileException $exception) {
            $posthog->capture($user->email, 'l402.paywall_delete_reconcile_failed', [
                'paywallId' => $paywallId,
                'error' => $exception->getMessage(),
            ]);
            return $this->reconcileFailureResponse($exception);
        }

        /** @var L402Paywall $deleted */
        $deleted = $result['paywall'];
        $posthog->capture($user->email, 'l402.paywall_deleted', [
            'paywallId' => (string) $deleted->id,
            'name' => (string) $deleted->name,
            'deletedAt' => $deleted->deleted_at?->toISOString(),
        ]);

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

        $normalized = trim($value);

        if ($mustStartWithPathAnchor && ! str_starts_with($normalized, '^/')) {
            $fail("The {$attribute} must start with '^/' to scope path matching.");

            return;
        }

        if (! $mustStartWithPathAnchor && preg_match('/[a-z0-9](?:\\\.|\.)[a-z0-9]/i', $normalized) !== 1) {
            $fail("The {$attribute} must include an explicit host pattern.");

            return;
        }

        $candidate = '/'.str_replace('/', '\\/', $normalized).'/';

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
