<?php

namespace App\AI\Runtime;

use Illuminate\Contracts\Auth\Authenticatable;
use Symfony\Component\HttpFoundation\StreamedResponse;

final class RoutedRuntimeClient implements RuntimeClient
{
    private string $lastDriverName;

    public function __construct(
        private readonly LegacyRuntimeClient $legacyClient,
        private readonly ElixirRuntimeClient $elixirClient,
        private readonly RuntimeDriverRouter $router,
    ) {
        $defaultDriver = (string) config('runtime.driver', 'legacy');
        $this->lastDriverName = in_array($defaultDriver, ['legacy', 'elixir'], true) ? $defaultDriver : 'legacy';
    }

    public function driverName(): string
    {
        return $this->lastDriverName;
    }

    public function streamAutopilotRun(
        Authenticatable $user,
        string $threadId,
        string $prompt,
        bool $authenticatedSession = true,
    ): StreamedResponse {
        $driver = $this->router->resolveDriver(
            userId: (int) $user->getAuthIdentifier(),
            threadId: $threadId,
        );

        $this->lastDriverName = $driver;

        return $this->clientForDriver($driver)->streamAutopilotRun(
            user: $user,
            threadId: $threadId,
            prompt: $prompt,
            authenticatedSession: $authenticatedSession,
        );
    }

    private function clientForDriver(string $driver): RuntimeClient
    {
        if ($driver === 'elixir') {
            return $this->elixirClient;
        }

        if ((bool) config('runtime.shadow.enabled', false)) {
            return new ShadowRuntimeClient($this->legacyClient, $this->elixirClient);
        }

        return $this->legacyClient;
    }
}
