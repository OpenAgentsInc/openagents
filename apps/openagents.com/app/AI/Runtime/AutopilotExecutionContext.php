<?php

namespace App\AI\Runtime;

final class AutopilotExecutionContext
{
    private ?int $userId = null;

    private ?string $autopilotId = null;

    public function set(?int $userId, ?string $autopilotId): void
    {
        $this->userId = $this->normalizeUserId($userId);
        $this->autopilotId = $this->normalizeAutopilotId($autopilotId);
    }

    public function clear(): void
    {
        $this->userId = null;
        $this->autopilotId = null;
    }

    public function userId(): ?int
    {
        return $this->userId;
    }

    public function autopilotId(): ?string
    {
        return $this->autopilotId;
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
