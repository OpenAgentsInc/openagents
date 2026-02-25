<?php

namespace Laravel\WorkOS\Http\Middleware;

use Closure;
use Illuminate\Http\Request;
use Illuminate\Support\Facades\Auth;
use Laravel\WorkOS\WorkOS;
use Symfony\Component\HttpFoundation\RedirectResponse;
use WorkOS\Exception\WorkOSException;

class ValidateSessionWithWorkOS
{
    /**
     * Handle an incoming request.
     */
    public function handle(Request $request, Closure $next)
    {
        if (app()->runningUnitTests()) {
            return $next($request);
        }

        WorkOS::configure();

        if (! $request->session()->get('workos_access_token') ||
            ! $request->session()->get('workos_refresh_token')) {
            return $this->logout($request);
        }

        try {
            [$accessToken, $refreshToken] = WorkOS::ensureAccessTokenIsValid(
                $request->session()->get('workos_access_token'),
                $request->session()->get('workos_refresh_token'),
            );

            $request->session()->put('workos_access_token', $accessToken);
            $request->session()->put('workos_refresh_token', $refreshToken);
        } catch (WorkOSException $e) {
            report($e);

            return $this->logout($request);
        }

        return $next($request);
    }

    /**
     * Log the user out of the application.
     */
    protected function logout(Request $request): RedirectResponse
    {
        Auth::guard('web')->logout();

        $request->session()->invalidate();
        $request->session()->regenerateToken();

        return redirect('/');
    }
}
