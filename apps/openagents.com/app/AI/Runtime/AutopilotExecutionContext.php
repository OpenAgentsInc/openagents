<?php

namespace App\AI\Runtime;

final class AutopilotExecutionContext
{
    private ?int $userId = null;

    private ?string $autopilotId = null;

    /**
     * True when the current request session is authenticated as a real user.
     * False for guest/onboarding chat sessions.
     */
    private bool $authenticatedSession = true;

    public function set(?int $userId, ?string $autopilotId, bool $authenticatedSession = true): void
    {
        $this->userId = $this->normalizeUserId($userId);
        $this->autopilotId = $this->normalizeAutopilotId($autopilotId);
        $this->authenticatedSession = $authenticatedSession;
    }

    public function clear(): void
    {
        $this->userId = null;
        $this->autopilotId = null;
        $this->authenticatedSession = true;
    }

    public function userId(): ?int
    {
        return $this->userId;
    }

    public function autopilotId(): ?string
    {
        return $this->autopilotId;
    }

    public function authenticatedSession(): bool
    {
        return $this->authenticatedSession;
    }

    private function normalizeUserId(?int $userId): ?int
    {
        if (! is_int($userId) || $userId <= 0) {
            return null;
        }

        return $userId;
    }

    private function normalizeAutopilotId(?string $autopilotId): ?string
    {
        if (! is_string($autopilotId)) {
            return null;
        }

        $trimmed = trim($autopilotId);

        return $trimmed === '' ? null : $trimmed;
    }
}
