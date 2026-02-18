<?php

namespace App\Http\Middleware;

use App\Models\User;
use App\Support\AdminAccess;
use App\Support\ChatThreadList;
use Illuminate\Http\Request;
use Illuminate\Support\Facades\Auth;
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
        $user = $this->resolveInertiaUser($request);
        $threadList = app(ChatThreadList::class);

        return [
            ...parent::share($request),
            'name' => config('app.name'),
            'csrfToken' => $request->session()->token(),
            'auth' => [
                'user' => $user,
            ],
            'isAdmin' => AdminAccess::isAdminEmail($user?->email),
            'sidebarOpen' => ! $request->hasCookie('sidebar_state') || $request->cookie('sidebar_state') === 'true',
            'chatThreads' => $user
                ? $threadList->forUser((int) $user->id, 50)
                    ->map(fn ($thread) => [
                        'id' => (string) $thread->id,
                        'title' => $threadList->normalizeTitle($thread->title),
                        'updatedAt' => $thread->updated_at,
                    ])
                    ->all()
                : [],
        ];
    }

    private function resolveInertiaUser(Request $request): ?User
    {
        $user = $request->user();
        if ($user instanceof User) {
            return $user;
        }

        if (! $request->hasSession()) {
            return null;
        }

        $userId = (int) $request->session()->get('chat.auth_user_id', 0);
        if ($userId <= 0) {
            return null;
        }

        $rehydrated = User::query()->find($userId);
        if (! $rehydrated instanceof User) {
            $request->session()->forget('chat.auth_user_id');

            return null;
        }

        Auth::guard('web')->login($rehydrated);
        $request->setUserResolver(static fn (): User => $rehydrated);

        return $rehydrated;
    }
}
