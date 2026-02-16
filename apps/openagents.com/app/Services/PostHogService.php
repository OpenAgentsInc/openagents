<?php

namespace App\Services;

use Illuminate\Support\Facades\Auth;
use PostHog\PostHog;

class PostHogService
{
    protected static bool $initialized = false;

    public function __construct()
    {
        if (config('posthog.disabled')) {
            return;
        }

        // Initialize PostHog once
        if (! self::$initialized) {
            PostHog::init(
                config('posthog.api_key'),
                [
                    'host' => config('posthog.host'),
                    'debug' => config('posthog.debug'),
                ]
            );
            self::$initialized = true;
        }
    }

    public function identify(string $distinctId, array $properties = []): void
    {
        if (config('posthog.disabled')) {
            return;
        }

        PostHog::identify([
            'distinctId' => $distinctId,
            'properties' => $properties,
        ]);
    }

    public function capture(string $distinctId, string $event, array $properties = []): void
    {
        if (config('posthog.disabled')) {
            return;
        }

        PostHog::capture([
            'distinctId' => $distinctId,
            'event' => $event,
            'properties' => $properties,
        ]);
    }

    public function captureException(\Throwable $exception, ?string $distinctId = null): ?string
    {
        if (config('posthog.disabled')) {
            return null;
        }

        $distinctId = $distinctId ?? Auth::user()?->email ?? 'anonymous';

        $eventId = uniqid('error_', true);

        $this->capture($distinctId, '$exception', [
            'error_id' => $eventId,
            'exception_type' => get_class($exception),
            'exception_message' => $exception->getMessage(),
            'exception_file' => $exception->getFile(),
            'exception_line' => $exception->getLine(),
            'stack_trace' => $exception->getTraceAsString(),
        ]);

        return $eventId;
    }

    public function isFeatureEnabled(string $key, string $distinctId, array $properties = []): ?bool
    {
        if (config('posthog.disabled')) {
            return false;
        }

        return PostHog::isFeatureEnabled($key, $distinctId, $properties);
    }

    public function getFeatureFlagPayload(string $key, string $distinctId): mixed
    {
        if (config('posthog.disabled')) {
            return null;
        }

        return PostHog::getFeatureFlagPayload($key, $distinctId);
    }
}
