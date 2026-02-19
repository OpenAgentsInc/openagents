<?php

namespace App\Http\Middleware;

use App\Services\PostHogService;
use Closure;
use Illuminate\Http\Request;
use Symfony\Component\HttpFoundation\Response;

class CapturePostHogPageview
{
    public function handle(Request $request, Closure $next): Response
    {
        $response = $next($request);

        if (! $this->shouldCapture($request, $response)) {
            return $response;
        }

        // Only send server-side pageview for authenticated users. Unauthenticated
        // traffic is counted by client-side posthog-js when a real browser loads
        // the app, avoiding inflation from bots/crawlers (no session cookie, no JS).
        $user = $request->user();
        if ($user?->email === null || $user?->email === '') {
            return $response;
        }

        /** @var PostHogService $posthog */
        $posthog = app(PostHogService::class);
        $posthog->capture($user->email, '$pageview', [
            '$current_url' => $request->fullUrl(),
            '$pathname' => '/'.ltrim((string) $request->path(), '/'),
            'host' => $request->getHost(),
            'source' => 'laravel_web_middleware',
            'auth_state' => 'authenticated',
        ]);

        return $response;
    }

    private function shouldCapture(Request $request, Response $response): bool
    {
        if (! $request->isMethod('GET')) {
            return false;
        }

        if ($request->is('api/*') || $request->is('build/*')) {
            return false;
        }

        if ($response->getStatusCode() >= 400) {
            return false;
        }

        $contentType = strtolower((string) $response->headers->get('Content-Type', ''));
        $isHtml = str_contains($contentType, 'text/html');
        $isInertia = $request->headers->has('X-Inertia');

        return $isHtml || $isInertia;
    }
}
