<?php

namespace App\Http\Controllers\Api;

use App\Http\Controllers\Controller;
use App\Support\AdminAccess;
use Illuminate\Http\JsonResponse;
use Illuminate\Http\Request;
use Illuminate\Support\Facades\DB;

class MeController extends Controller
{
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
