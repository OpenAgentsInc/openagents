<?php

use Illuminate\Support\Facades\Route;

it('has endpoint coverage manifest for all API routes', function () {
    $actual = collect(Route::getRoutes()->getRoutes())
        ->flatMap(function ($route): array {
            $uri = $route->uri();
            if (! str_starts_with($uri, 'api/')) {
                return [];
            }

            return collect($route->methods())
                ->reject(fn (string $method) => in_array($method, ['HEAD', 'OPTIONS'], true))
                ->map(fn (string $method) => sprintf('%s %s', $method, $uri))
                ->values()
                ->all();
        })
        ->unique()
        ->sort()
        ->values()
        ->all();

    $expected = [
        'DELETE api/l402/paywalls/{paywallId}',
        'DELETE api/settings/profile',
        'DELETE api/tokens',
        'DELETE api/tokens/current',
        'DELETE api/tokens/{tokenId}',
        'GET api/agent-payments/balance',
        'GET api/agent-payments/wallet',
        'GET api/agents/me/balance',
        'GET api/agents/me/wallet',
        'GET api/autopilots',
        'GET api/autopilots/{autopilot}',
        'GET api/autopilots/{autopilot}/threads',
        'GET api/chat/guest-session',
        'GET api/chats',
        'GET api/chats/{conversationId}',
        'GET api/chats/{conversationId}/messages',
        'GET api/chats/{conversationId}/runs',
        'GET api/chats/{conversationId}/runs/{runId}/events',
        'GET api/l402/deployments',
        'GET api/l402/paywalls',
        'GET api/l402/settlements',
        'GET api/l402/transactions',
        'GET api/l402/transactions/{eventId}',
        'GET api/l402/wallet',
        'GET api/me',
        'GET api/settings/profile',
        'GET api/shouts',
        'GET api/shouts/zones',
        'GET api/smoke/stream',
        'GET api/tokens',
        'GET api/whispers',
        'PATCH api/autopilots/{autopilot}',
        'PATCH api/l402/paywalls/{paywallId}',
        'PATCH api/settings/profile',
        'PATCH api/whispers/{id}/read',
        'POST api/agent-payments/invoice',
        'POST api/agent-payments/pay',
        'POST api/agent-payments/send-spark',
        'POST api/agent-payments/wallet',
        'POST api/agents/me/wallet',
        'POST api/auth/email',
        'POST api/auth/register',
        'POST api/auth/verify',
        'POST api/autopilots',
        'POST api/autopilots/{autopilot}/stream',
        'POST api/autopilots/{autopilot}/threads',
        'POST api/chat',
        'POST api/chat/stream',
        'POST api/chats',
        'POST api/chats/{conversationId}/stream',
        'POST api/internal/runtime/integrations/secrets/fetch',
        'POST api/l402/paywalls',
        'POST api/payments/invoice',
        'POST api/payments/pay',
        'POST api/payments/send-spark',
        'POST api/shouts',
        'POST api/tokens',
        'POST api/webhooks/resend',
        'POST api/whispers',
    ];

    expect($actual)->toBe($expected);
});
