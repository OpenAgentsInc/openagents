<?php

namespace App\Http\Controllers\Api;

use App\Http\Controllers\Controller;
use App\OpenApi\Parameters\ChatLimitQueryParameter;
use App\OpenApi\Responses\MeResponse;
use App\OpenApi\Responses\UnauthorizedResponse;
use App\Support\AdminAccess;
use Illuminate\Http\JsonResponse;
use Illuminate\Http\Request;
use Illuminate\Support\Facades\DB;
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

        $threads = DB::table('threads')
            ->where('user_id', $user->id)
            ->orderByDesc('updated_at')
            ->limit($limit)
            ->get(['id', 'title', 'updated_at'])
            ->map(fn ($thread): array => [
                'id' => (string) $thread->id,
                'title' => (string) ($thread->title ?: 'New conversation'),
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
