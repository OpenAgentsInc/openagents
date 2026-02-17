<?php

namespace App\Http\Controllers\Api;

use App\Http\Controllers\Controller;
use App\Http\Requests\Api\CreateTokenRequest;
use App\OpenApi\RequestBodies\CreateTokenRequestBody;
use App\OpenApi\Responses\DataObjectResponse;
use App\OpenApi\Responses\NotFoundResponse;
use App\OpenApi\Responses\TokenCreateResponse;
use App\OpenApi\Responses\TokenListResponse;
use App\OpenApi\Responses\UnauthorizedResponse;
use App\OpenApi\Responses\ValidationErrorResponse;
use Illuminate\Http\JsonResponse;
use Illuminate\Http\Request;
use Illuminate\Support\Carbon;
use Laravel\Sanctum\PersonalAccessToken;
use Vyuldashev\LaravelOpenApi\Attributes as OpenApi;

#[OpenApi\PathItem]
class TokenController extends Controller
{
    /**
     * List personal access tokens.
     *
     * Returns Sanctum tokens owned by the authenticated user, including whether
     * each token is the current one.
     */
    #[OpenApi\Operation(tags: ['Auth'])]
    #[OpenApi\Response(factory: TokenListResponse::class, statusCode: 200)]
    #[OpenApi\Response(factory: UnauthorizedResponse::class, statusCode: 401)]
    public function index(Request $request): JsonResponse
    {
        $user = $request->user();
        if (! $user) {
            abort(401);
        }

        $currentAccessTokenId = $user->currentAccessToken()?->id;

        $tokens = $user->tokens()
            ->orderByDesc('id')
            ->get()
            ->map(fn ($token): array => [
                'id' => (int) $token->id,
                'name' => (string) $token->name,
                'abilities' => is_array($token->abilities) ? $token->abilities : [],
                'lastUsedAt' => $token->last_used_at?->toISOString(),
                'expiresAt' => $token->expires_at?->toISOString(),
                'createdAt' => $token->created_at?->toISOString(),
                'isCurrent' => $currentAccessTokenId !== null && (int) $currentAccessTokenId === (int) $token->id,
            ])
            ->values()
            ->all();

        return response()->json(['data' => $tokens]);
    }

    /**
     * Create a personal access token.
     *
     * Returns the plain-text token once. Store it securely; it cannot be read
     * again from the API.
     */
    #[OpenApi\Operation(tags: ['Auth'])]
    #[OpenApi\RequestBody(factory: CreateTokenRequestBody::class)]
    #[OpenApi\Response(factory: TokenCreateResponse::class, statusCode: 201)]
    #[OpenApi\Response(factory: UnauthorizedResponse::class, statusCode: 401)]
    #[OpenApi\Response(factory: ValidationErrorResponse::class, statusCode: 422)]
    public function store(CreateTokenRequest $request): JsonResponse
    {
        $user = $request->user();
        if (! $user) {
            abort(401);
        }

        $validated = $request->validated();

        $abilities = ['*'];
        if (isset($validated['abilities']) && is_array($validated['abilities']) && $validated['abilities'] !== []) {
            $abilities = array_values(array_map(
                static fn (string $ability): string => trim($ability),
                array_filter($validated['abilities'], static fn ($ability): bool => is_string($ability) && trim($ability) !== ''),
            ));

            if ($abilities === []) {
                $abilities = ['*'];
            }
        }

        $expiresAt = null;
        if (isset($validated['expires_at']) && is_string($validated['expires_at']) && trim($validated['expires_at']) !== '') {
            $expiresAt = Carbon::parse($validated['expires_at']);
        }

        $newToken = $user->createToken(
            name: (string) $validated['name'],
            abilities: $abilities,
            expiresAt: $expiresAt,
        );

        return response()->json([
            'data' => [
                'token' => $newToken->plainTextToken,
                'tokenableId' => (int) $user->id,
                'name' => (string) $validated['name'],
                'abilities' => $abilities,
                'expiresAt' => $expiresAt?->toISOString(),
            ],
        ], 201);
    }

    /**
     * Revoke a specific personal access token.
     */
    #[OpenApi\Operation(tags: ['Auth'])]
    #[OpenApi\Response(factory: DataObjectResponse::class, statusCode: 200)]
    #[OpenApi\Response(factory: UnauthorizedResponse::class, statusCode: 401)]
    #[OpenApi\Response(factory: NotFoundResponse::class, statusCode: 404)]
    public function destroy(Request $request, int $tokenId): JsonResponse
    {
        $user = $request->user();
        if (! $user) {
            abort(401);
        }

        $deleted = $user->tokens()->where('id', $tokenId)->delete();

        if ($deleted === 0) {
            abort(404);
        }

        return response()->json(['data' => ['deleted' => true]]);
    }

    /**
     * Revoke the current bearer token.
     */
    #[OpenApi\Operation(tags: ['Auth'])]
    #[OpenApi\Response(factory: DataObjectResponse::class, statusCode: 200)]
    #[OpenApi\Response(factory: UnauthorizedResponse::class, statusCode: 401)]
    public function destroyCurrent(Request $request): JsonResponse
    {
        $user = $request->user();
        if (! $user) {
            abort(401);
        }

        $deleted = false;

        $current = $user->currentAccessToken();
        if ($current) {
            $deleted = $user->tokens()->whereKey($current->getKey())->delete() > 0;
        }

        if (! $deleted) {
            $bearer = (string) $request->bearerToken();
            if ($bearer !== '') {
                $resolved = PersonalAccessToken::findToken($bearer);
                if ($resolved && (int) $resolved->tokenable_id === (int) $user->id) {
                    $deleted = $user->tokens()->whereKey($resolved->getKey())->delete() > 0;
                }
            }
        }

        return response()->json(['data' => ['deleted' => $deleted]]);
    }

    /**
     * Revoke all personal access tokens for the authenticated user.
     */
    #[OpenApi\Operation(tags: ['Auth'])]
    #[OpenApi\Response(factory: DataObjectResponse::class, statusCode: 200)]
    #[OpenApi\Response(factory: UnauthorizedResponse::class, statusCode: 401)]
    public function destroyAll(Request $request): JsonResponse
    {
        $user = $request->user();
        if (! $user) {
            abort(401);
        }

        $count = $user->tokens()->count();
        $user->tokens()->delete();

        return response()->json(['data' => ['deletedCount' => $count]]);
    }
}
