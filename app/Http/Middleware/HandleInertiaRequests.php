<?php

namespace App\Http\Middleware;

use App\Models\Thread;
use Illuminate\Http\Request;
use Inertia\Middleware;

class HandleInertiaRequests extends Middleware
{
    /**
     * The root template that is loaded on the first page visit.
     *
     * @var string
     */
    protected $rootView = 'app';

    /**
     * Determine the current asset version.
     */
    public function version(Request $request): ?string
    {
        return parent::version($request);
    }

    /**
     * Define the props that are shared by default.
     *
     * @return array<string, mixed>
     */
    public function share(Request $request): array
    {
        $user = $request->user();
        
        return [
            ...parent::share($request),
            'auth' => [
                'user' => $user,
                'teams' => $user ? $user->teams : [],
                'current_team' => $user ? $user->currentTeam : null,
            ],
            'threads' => $user ? Thread::where('user_id', $user->id)
                ->orderBy('created_at', 'desc')
                ->get() : [],
        ];
    }
}