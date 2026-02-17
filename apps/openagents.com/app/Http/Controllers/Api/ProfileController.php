<?php

namespace App\Http\Controllers\Api;

use App\Http\Controllers\Controller;
use App\Http\Requests\Api\DeleteProfileRequest;
use App\Http\Requests\Settings\ProfileUpdateRequest;
use App\Models\User;
use App\OpenApi\RequestBodies\DeleteProfileRequestBody;
use App\OpenApi\RequestBodies\ProfileUpdateRequestBody;
use App\OpenApi\Responses\DataObjectResponse;
use App\OpenApi\Responses\ProfileResponse;
use App\OpenApi\Responses\UnauthorizedResponse;
use App\OpenApi\Responses\ValidationErrorResponse;
use App\Services\PostHogService;
use Illuminate\Http\JsonResponse;
use Illuminate\Http\Request;
use Vyuldashev\LaravelOpenApi\Attributes as OpenApi;

#[OpenApi\PathItem]
class ProfileController extends Controller
{
    /**
     * Get the authenticated user's profile.
     */
    #[OpenApi\Operation(tags: ['Profile'])]
    #[OpenApi\Response(factory: ProfileResponse::class, statusCode: 200)]
    #[OpenApi\Response(factory: UnauthorizedResponse::class, statusCode: 401)]
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

    /**
     * Update profile fields.
     *
     * Currently supports `name` updates.
     */
    #[OpenApi\Operation(tags: ['Profile'])]
    #[OpenApi\RequestBody(factory: ProfileUpdateRequestBody::class)]
    #[OpenApi\Response(factory: ProfileResponse::class, statusCode: 200)]
    #[OpenApi\Response(factory: UnauthorizedResponse::class, statusCode: 401)]
    #[OpenApi\Response(factory: ValidationErrorResponse::class, statusCode: 422)]
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

    /**
     * Delete the authenticated account.
     *
     * Requires confirming the currently authenticated email address.
     */
    #[OpenApi\Operation(tags: ['Profile'])]
    #[OpenApi\RequestBody(factory: DeleteProfileRequestBody::class)]
    #[OpenApi\Response(factory: DataObjectResponse::class, statusCode: 200)]
    #[OpenApi\Response(factory: UnauthorizedResponse::class, statusCode: 401)]
    #[OpenApi\Response(factory: ValidationErrorResponse::class, statusCode: 422)]
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
