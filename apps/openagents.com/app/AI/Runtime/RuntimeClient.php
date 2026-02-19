<?php

namespace App\AI\Runtime;

use Illuminate\Contracts\Auth\Authenticatable;
use Symfony\Component\HttpFoundation\StreamedResponse;

interface RuntimeClient
{
    public function driverName(): string;

    public function streamAutopilotRun(
        Authenticatable $user,
        string $threadId,
        string $prompt,
        bool $authenticatedSession = true,
    ): StreamedResponse;
}
