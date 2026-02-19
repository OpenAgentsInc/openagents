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

        $distinctId = $request->user()?->email;

        if (! is_string($distinctId) || $distinctId === '') {
            $sessionId = (string) $request->session()->getId();
            $distinctId = $sessionId !== '' ? 'guest:'.$sessionId : 'guest:'.sha1((string) $request->ip());
        }

        /** @var PostHogService $posthog */
        $posthog = app(PostHogService::class);
        $posthog->capture($distinctId, '$pageview', [
            '$current_url' => $request->fullUrl(),
            '$pathname' => '/'.ltrim((string) $request->path(), '/'),
            'host' => $request->getHost(),
            'source' => 'laravel_web_middleware',
            'auth_state' => $request->user() ? 'authenticated' : 'guest',
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
