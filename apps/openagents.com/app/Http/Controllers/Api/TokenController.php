<?php

namespace App\Http\Controllers\Api;

use App\Http\Controllers\Controller;
use App\Http\Requests\Api\CreateTokenRequest;
use Illuminate\Http\JsonResponse;
use Illuminate\Http\Request;
use Illuminate\Support\Carbon;

class TokenController extends Controller
{
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

    public function destroyCurrent(Request $request): JsonResponse
    {
        $user = $request->user();
        if (! $user) {
            abort(401);
        }

        $current = $user->currentAccessToken();
        if ($current) {
            $current->delete();
        }

        return response()->json(['data' => ['deleted' => $current !== null]]);
    }

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
