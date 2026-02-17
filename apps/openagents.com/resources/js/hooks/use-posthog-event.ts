import { usePostHog } from '@posthog/react';
import { useCallback } from 'react';

type EventProperties = Record<string, unknown>;

/**
 * Lightweight event helper that no-ops when PostHog is unavailable.
 */
export function usePostHogEvent(namespace: string) {
    const posthog = usePostHog();

    return useCallback(
        (event: string, properties: EventProperties = {}) => {
            if (!posthog) return;

            posthog.capture(event, {
                namespace,
                ...properties,
            });
        },
        [posthog, namespace],
    );
}
