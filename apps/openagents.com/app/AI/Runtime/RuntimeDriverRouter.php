<?php

namespace App\AI\Runtime;

use Illuminate\Support\Facades\DB;

final class RuntimeDriverRouter
{
    /**
     * Resolve the effective runtime driver for a run request.
     */
    public function resolveDriver(
        int $userId,
        string $threadId,
        ?string $autopilotId = null,
    ): string {
        if ((bool) config('runtime.rollback.force_legacy', false)) {
            return 'legacy';
        }

        $forcedDriver = (string) (config('runtime.force_driver') ?? '');
        if (in_array($forcedDriver, ['legacy', 'elixir'], true)) {
            return $forcedDriver;
        }

        $defaultDriver = (string) config('runtime.driver', 'legacy');
        if (! in_array($defaultDriver, ['legacy', 'elixir'], true)) {
            $defaultDriver = 'legacy';
        }

        $autopilotId = $autopilotId ?: $this->autopilotIdForThread($threadId);

        if ((bool) config('runtime.overrides.enabled', true)) {
            $userOverride = $this->overrideDriverForScope('user', (string) $userId);
            if ($userOverride !== null) {
                return $userOverride;
            }

            if (is_string($autopilotId) && $autopilotId !== '') {
                $autopilotOverride = $this->overrideDriverForScope('autopilot', $autopilotId);
                if ($autopilotOverride !== null) {
                    return $autopilotOverride;
                }
            }
        }

        if (is_string($autopilotId) && $autopilotId !== '') {
            $boundDriver = $this->autopilotBindingDriver($autopilotId);
            if ($boundDriver !== null) {
                return $boundDriver;
            }
        }

        $userCanaryPercent = $this->clampPercent((int) config('runtime.canary.user_percent', 0));
        if ($userCanaryPercent > 0 && $this->inCanary('user', (string) $userId, $userCanaryPercent)) {
            return 'elixir';
        }

        $autopilotCanaryPercent = $this->clampPercent((int) config('runtime.canary.autopilot_percent', 0));
        if (
            $autopilotCanaryPercent > 0
            && is_string($autopilotId)
            && $autopilotId !== ''
            && $this->inCanary('autopilot', $autopilotId, $autopilotCanaryPercent)
        ) {
            return 'elixir';
        }

        return $defaultDriver;
    }

    public function autopilotIdForThread(string $threadId): ?string
    {
        $autopilotId = DB::table('threads')
            ->where('id', $threadId)
            ->value('autopilot_id');

        if (! is_string($autopilotId) || trim($autopilotId) === '') {
            return null;
        }

        return $autopilotId;
    }

    private function overrideDriverForScope(string $scopeType, string $scopeId): ?string
    {
        $override = DB::table('runtime_driver_overrides')
            ->where('scope_type', $scopeType)
            ->where('scope_id', $scopeId)
            ->where('is_active', true)
            ->first(['driver']);

        if (! $override) {
            return null;
        }

        $driver = (string) $override->driver;

        return in_array($driver, ['legacy', 'elixir'], true) ? $driver : null;
    }

    private function autopilotBindingDriver(string $autopilotId): ?string
    {
        $runtimeType = DB::table('autopilot_runtime_bindings')
            ->where('autopilot_id', $autopilotId)
            ->where('is_primary', true)
            ->value('runtime_type');

        if (! is_string($runtimeType)) {
            return null;
        }

        $runtimeType = strtolower(trim($runtimeType));

        return match ($runtimeType) {
            'elixir', 'runtime' => 'elixir',
            'legacy', 'laravel', 'openagents.com' => 'legacy',
            default => null,
        };
    }

    private function inCanary(string $scopeType, string $scopeId, int $percent): bool
    {
        $seed = (string) config('runtime.canary.seed', 'runtime-canary-v1');
        $hash = hash('sha256', "{$seed}|{$scopeType}|{$scopeId}");
        $bucket = hexdec(substr($hash, 0, 8)) % 100;

        return $bucket < $percent;
    }

    private function clampPercent(int $percent): int
    {
        return max(0, min(100, $percent));
    }
}
