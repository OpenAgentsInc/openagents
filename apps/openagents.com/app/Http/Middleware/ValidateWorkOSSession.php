<?php

namespace App\Http\Middleware;

use App\Models\User;
use Closure;
use Illuminate\Http\Request;
use Laravel\WorkOS\Http\Middleware\ValidateSessionWithWorkOS;
use Symfony\Component\HttpFoundation\Response;

class ValidateWorkOSSession
{
    public function handle(Request $request, Closure $next): Response
    {
        if ($this->shouldBypassWorkOsValidation($request)) {
            return $next($request);
        }

        return app(ValidateSessionWithWorkOS::class)->handle($request, $next);
    }

    private function shouldBypassWorkOsValidation(Request $request): bool
    {
        if (! config('auth.local_test_login.enabled', false)) {
            return false;
        }

        if ($request->session()->get('oa_local_test_auth') !== true) {
            return false;
        }

        $user = $request->user();
        if (! $user instanceof User) {
            return false;
        }

        return str_starts_with((string) $user->workos_id, 'test_local_');
    }
}
