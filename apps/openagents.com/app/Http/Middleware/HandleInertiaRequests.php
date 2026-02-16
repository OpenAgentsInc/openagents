<?php

namespace App\Http\Middleware;

use Illuminate\Http\Request;
use Illuminate\Support\Facades\DB;
use Inertia\Middleware;

class HandleInertiaRequests extends Middleware
{
    /**
     * The root template that's loaded on the first page visit.
     *
     * @see https://inertiajs.com/server-side-setup#root-template
     *
     * @var string
     */
    protected $rootView = 'app';

    /**
     * Determines the current asset version.
     *
     * @see https://inertiajs.com/asset-versioning
     */
    public function version(Request $request): ?string
    {
        return parent::version($request);
    }

    /**
     * Define the props that are shared by default.
     *
     * @see https://inertiajs.com/shared-data
     *
     * @return array<string, mixed>
     */
    public function share(Request $request): array
    {
        $user = $request->user();

        return [
            ...parent::share($request),
            'name' => config('app.name'),
            'auth' => [
                'user' => $user,
            ],
            'sidebarOpen' => ! $request->hasCookie('sidebar_state') || $request->cookie('sidebar_state') === 'true',
            'chatThreads' => $user
                ? DB::table('threads')
                    ->where('user_id', $user->id)
                    ->orderByDesc('updated_at')
                    ->limit(50)
                    ->get(['id', 'title', 'updated_at'])
                    ->map(fn ($thread) => [
                        'id' => (string) $thread->id,
                        'title' => (string) ($thread->title ?: 'New conversation'),
                        'updatedAt' => $thread->updated_at,
                    ])
                    ->all()
                : [],
        ];
    }
}
