<?php

namespace App\AI\Runtime;

use App\AI\RunOrchestrator;
use Illuminate\Contracts\Auth\Authenticatable;
use Symfony\Component\HttpFoundation\StreamedResponse;

final class LegacyRuntimeClient implements RuntimeClient
{
    public function __construct(
        private readonly RunOrchestrator $runOrchestrator,
    ) {}

    public function driverName(): string
    {
        return 'legacy';
    }

    public function streamAutopilotRun(
        Authenticatable $user,
        string $threadId,
        string $prompt,
        bool $authenticatedSession = true,
    ): StreamedResponse {
        return $this->runOrchestrator->streamAutopilotRun(
            user: $user,
            threadId: $threadId,
            prompt: $prompt,
            authenticatedSession: $authenticatedSession,
        );
    }
}
