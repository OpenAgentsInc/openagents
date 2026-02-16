<?php

namespace App\Http\Controllers\Api;

use App\Http\Controllers\Controller;
use App\Http\Requests\Api\DeleteProfileRequest;
use App\Http\Requests\Settings\ProfileUpdateRequest;
use App\Models\User;
use App\Services\PostHogService;
use Illuminate\Http\JsonResponse;
use Illuminate\Http\Request;

class ProfileController extends Controller
{
    public function show(Request $request): JsonResponse
    {
        $user = $request->user();
        if (! $user) {
            abort(401);
        }

        return response()->json([
            'data' => [
                'id' => (int) $user->id,
                'name' => (string) $user->name,
                'email' => (string) $user->email,
                'avatar' => (string) $user->avatar,
                'createdAt' => $user->created_at?->toISOString(),
                'updatedAt' => $user->updated_at?->toISOString(),
            ],
        ]);
    }

    public function update(ProfileUpdateRequest $request, PostHogService $posthog): JsonResponse
    {
        $user = $request->user();
        if (! $user) {
            abort(401);
        }

        $user->update([
            'name' => (string) $request->validated()['name'],
        ]);

        $posthog->capture($user->email, 'profile updated', [
            'field_updated' => 'name',
            'source' => 'api',
        ]);

        return response()->json([
            'data' => [
                'id' => (int) $user->id,
                'name' => (string) $user->name,
                'email' => (string) $user->email,
                'avatar' => (string) $user->avatar,
                'updatedAt' => $user->updated_at?->toISOString(),
            ],
        ]);
    }

    public function destroy(DeleteProfileRequest $request, PostHogService $posthog): JsonResponse
    {
        $user = $request->user();
        if (! $user instanceof User) {
            abort(401);
        }

        $validatedEmail = strtolower(trim((string) $request->validated()['email']));
        $userEmail = strtolower(trim((string) $user->email));

        if (! hash_equals($userEmail, $validatedEmail)) {
            return response()->json([
                'message' => 'Email confirmation does not match the authenticated user.',
                'errors' => [
                    'email' => ['Email confirmation does not match the authenticated user.'],
                ],
            ], 422);
        }

        $posthog->capture($user->email, 'account deleted', [
            'source' => 'api',
        ]);

        $user->tokens()->delete();
        $user->delete();

        return response()->json([
            'data' => ['deleted' => true],
        ]);
    }
}
