<?php

namespace App\Http\Controllers\Api;

use App\Http\Controllers\Controller;
use App\OpenApi\Parameters\ChatLimitQueryParameter;
use App\OpenApi\Responses\MeResponse;
use App\OpenApi\Responses\UnauthorizedResponse;
use App\Support\AdminAccess;
use App\Support\ChatThreadList;
use Illuminate\Http\JsonResponse;
use Illuminate\Http\Request;
use Vyuldashev\LaravelOpenApi\Attributes as OpenApi;

#[OpenApi\PathItem]
class MeController extends Controller
{
    /**
     * Get authenticated user context.
     *
     * Returns current user profile fields, admin flag, and recent chat thread
     * summaries.
     */
    #[OpenApi\Operation(tags: ['Auth'])]
    #[OpenApi\Parameters(factory: ChatLimitQueryParameter::class)]
    #[OpenApi\Response(factory: MeResponse::class, statusCode: 200)]
    #[OpenApi\Response(factory: UnauthorizedResponse::class, statusCode: 401)]
    public function show(Request $request): JsonResponse
    {
        $user = $request->user();
        if (! $user) {
            abort(401);
        }

        $limit = max(1, min(200, (int) $request->integer('chat_limit', 50)));
        $threadList = app(ChatThreadList::class);

        $threads = $threadList->forUser((int) $user->id, $limit)
            ->map(fn ($thread): array => [
                'id' => (string) $thread->id,
                'title' => $threadList->normalizeTitle($thread->title),
                'updatedAt' => $thread->updated_at,
            ])
            ->all();

        return response()->json([
            'data' => [
                'user' => [
                    'id' => (int) $user->id,
                    'name' => (string) $user->name,
                    'email' => (string) $user->email,
                    'avatar' => (string) $user->avatar,
                    'createdAt' => $user->created_at?->toISOString(),
                    'updatedAt' => $user->updated_at?->toISOString(),
                ],
                'isAdmin' => AdminAccess::isAdminEmail($user->email),
                'chatThreads' => $threads,
            ],
        ]);
    }
}
