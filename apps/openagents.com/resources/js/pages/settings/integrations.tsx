import { Transition } from '@headlessui/react';
import { Form, Head, usePage } from '@inertiajs/react';
import { useEffect } from 'react';
import Heading from '@/components/heading';
import InputError from '@/components/input-error';
import { Button } from '@/components/ui/button';
import { Input } from '@/components/ui/input';
import { Label } from '@/components/ui/label';
import { usePostHogEvent } from '@/hooks/use-posthog-event';
import SettingsLayout from '@/layouts/settings/layout';

type Integration = {
    provider: string;
    status: string;
    connected: boolean;
    secretLast4?: string | null;
    connectedAt?: string | null;
    disconnectedAt?: string | null;
    metadata?: Record<string, string | null | undefined>;
};

type ResendDeliveryProjection = {
    provider: string;
    integrationId: string;
    lastState?: string | null;
    lastEventAt?: string | null;
    lastMessageId?: string | null;
    lastRecipient?: string | null;
    runtimeEventId?: string | null;
    source?: string | null;
};

type AuditItem = {
    action: string;
    createdAt?: string | null;
    metadata?: Record<string, string | null>;
};

type PageProps = {
    status?: string | null;
    integrations: {
        resend: Integration;
        google: Integration;
    };
    deliveryProjection?: {
        resend?: ResendDeliveryProjection | null;
    };
    integrationAudit?: {
        resend?: AuditItem[];
        google?: AuditItem[];
    };
};

export default function IntegrationsSettings() {
    const { status, integrations, deliveryProjection, integrationAudit } =
        usePage<PageProps>().props;

    const resend = integrations.resend;
    const google = integrations.google;
    const resendProjection = deliveryProjection?.resend ?? null;
    const resendAudit = integrationAudit?.resend ?? [];
    const googleAudit = integrationAudit?.google ?? [];

    const capture = usePostHogEvent('settings_integrations');

    useEffect(() => {
        capture('settings_integrations.page_opened', {
            resendConnected: resend.connected,
            resendStatus: resend.status,
            googleConnected: google.connected,
            googleStatus: google.status,
        });
    }, [
        capture,
        google.connected,
        google.status,
        resend.connected,
        resend.status,
    ]);

    return (
        <>
            <Head title="Integration settings" />

            <h1 className="sr-only">Integration Settings</h1>

            <SettingsLayout>
                <div className="space-y-6">
                    <Heading
                        variant="small"
                        title="Integrations"
                        description="Connect provider credentials used by runtime tools."
                    />

                    <div className="rounded-lg border border-border p-4">
                        <div className="mb-4 flex items-center justify-between gap-3">
                            <div>
                                <h2 className="text-base font-semibold">
                                    Google (Gmail)
                                </h2>
                                <p className="text-sm text-muted-foreground">
                                    Status:{' '}
                                    {google.connected
                                        ? 'Connected'
                                        : 'Not connected'}
                                    {google.secretLast4
                                        ? ` (refresh token ••••${google.secretLast4})`
                                        : ''}
                                </p>
                                {google.metadata?.scope ? (
                                    <p className="text-xs text-muted-foreground">
                                        Scopes: {google.metadata.scope}
                                    </p>
                                ) : null}
                            </div>
                            <span
                                className={`rounded-md px-2 py-1 text-xs font-medium ${
                                    google.connected
                                        ? 'bg-emerald-500/10 text-emerald-700 dark:text-emerald-300'
                                        : 'bg-zinc-500/10 text-zinc-700 dark:text-zinc-300'
                                }`}
                            >
                                {google.status}
                            </span>
                        </div>

                        <div className="flex flex-wrap items-center gap-3">
                            <Button asChild>
                                <a
                                    href="/settings/integrations/google/redirect"
                                    onClick={() => {
                                        capture(
                                            'settings_integrations.google_connect_clicked',
                                            {
                                                googleConnected:
                                                    google.connected,
                                            },
                                        );
                                    }}
                                >
                                    {google.connected
                                        ? 'Reconnect Google'
                                        : 'Connect Google'}
                                </a>
                            </Button>

                            <Form
                                action="/settings/integrations/google"
                                method="delete"
                                options={{ preserveScroll: true }}
                            >
                                {({ processing }) => (
                                    <Button
                                        type="submit"
                                        variant="destructive"
                                        disabled={
                                            processing || !google.connected
                                        }
                                        onClick={() => {
                                            capture(
                                                'settings_integrations.google_disconnect_submitted',
                                                {
                                                    googleConnected:
                                                        google.connected,
                                                },
                                            );
                                        }}
                                    >
                                        Disconnect
                                    </Button>
                                )}
                            </Form>

                            <Transition
                                show={
                                    status === 'google-connected' ||
                                    status === 'google-rotated' ||
                                    status === 'google-updated' ||
                                    status === 'google-disconnected'
                                }
                                enter="transition ease-in-out"
                                enterFrom="opacity-0"
                                leave="transition ease-in-out"
                                leaveTo="opacity-0"
                            >
                                <p className="text-sm text-zinc-600 dark:text-zinc-300">
                                    {status === 'google-disconnected'
                                        ? 'Google disconnected'
                                        : status === 'google-rotated'
                                          ? 'Google token rotated'
                                          : status === 'google-updated'
                                            ? 'Google integration updated'
                                            : 'Google connected'}
                                </p>
                            </Transition>
                        </div>

                        <div className="mt-6 border-t border-border pt-4">
                            <h3 className="mb-2 text-sm font-semibold">
                                Google lifecycle events
                            </h3>
                            {googleAudit.length === 0 ? (
                                <p className="text-sm text-muted-foreground">
                                    No events recorded yet.
                                </p>
                            ) : (
                                <ul className="space-y-1 text-sm text-muted-foreground">
                                    {googleAudit.map((item, index) => (
                                        <li
                                            key={`${item.action}-${item.createdAt ?? index}`}
                                        >
                                            <span className="font-medium text-foreground">
                                                {item.action}
                                            </span>{' '}
                                            {item.createdAt ? (
                                                <span>
                                                    at{' '}
                                                    {new Date(
                                                        item.createdAt,
                                                    ).toLocaleString()}
                                                </span>
                                            ) : null}
                                        </li>
                                    ))}
                                </ul>
                            )}
                        </div>
                    </div>

                    <div className="rounded-lg border border-border p-4">
                        <div className="mb-4 flex items-center justify-between gap-3">
                            <div>
                                <h2 className="text-base font-semibold">
                                    Resend
                                </h2>
                                <p className="text-sm text-muted-foreground">
                                    Status:{' '}
                                    {resend.connected
                                        ? 'Connected'
                                        : 'Not connected'}
                                    {resend.secretLast4
                                        ? ` (••••${resend.secretLast4})`
                                        : ''}
                                </p>
                                {resendProjection?.lastState ? (
                                    <p className="text-xs text-muted-foreground">
                                        Last delivery:{' '}
                                        {resendProjection.lastState}
                                        {resendProjection.lastEventAt
                                            ? ` at ${new Date(resendProjection.lastEventAt).toLocaleString()}`
                                            : ''}
                                    </p>
                                ) : null}
                            </div>
                            <span
                                className={`rounded-md px-2 py-1 text-xs font-medium ${
                                    resend.connected
                                        ? 'bg-emerald-500/10 text-emerald-700 dark:text-emerald-300'
                                        : 'bg-zinc-500/10 text-zinc-700 dark:text-zinc-300'
                                }`}
                            >
                                {resend.status}
                            </span>
                        </div>

                        <Form
                            action="/settings/integrations/resend"
                            method="post"
                            options={{ preserveScroll: true }}
                            className="space-y-4"
                            onSubmit={() => {
                                capture(
                                    'settings_integrations.resend_connect_submitted',
                                    {
                                        resendConnected: resend.connected,
                                    },
                                );
                            }}
                        >
                            {({ processing, recentlySuccessful, errors }) => (
                                <>
                                    <div className="grid gap-2">
                                        <Label htmlFor="resend-api-key">
                                            Resend API key
                                        </Label>
                                        <Input
                                            id="resend-api-key"
                                            name="resend_api_key"
                                            type="password"
                                            placeholder={
                                                resend.connected
                                                    ? 'Enter new key to rotate'
                                                    : 're_...'
                                            }
                                            autoComplete="off"
                                            required
                                        />
                                        <p className="text-xs text-muted-foreground">
                                            Stored encrypted. The full key is
                                            never shown again after save.
                                        </p>
                                        <InputError
                                            className="mt-1"
                                            message={errors.resend_api_key}
                                        />
                                    </div>

                                    <div className="grid gap-2">
                                        <Label htmlFor="resend-sender-email">
                                            Sender email
                                        </Label>
                                        <Input
                                            id="resend-sender-email"
                                            name="sender_email"
                                            type="email"
                                            defaultValue={
                                                resend.metadata?.sender_email ??
                                                ''
                                            }
                                            placeholder="noreply@yourdomain.com"
                                            autoComplete="off"
                                        />
                                        <InputError
                                            className="mt-1"
                                            message={errors.sender_email}
                                        />
                                    </div>

                                    <div className="grid gap-2">
                                        <Label htmlFor="resend-sender-name">
                                            Sender name
                                        </Label>
                                        <Input
                                            id="resend-sender-name"
                                            name="sender_name"
                                            defaultValue={
                                                resend.metadata?.sender_name ??
                                                ''
                                            }
                                            placeholder="OpenAgents"
                                            autoComplete="off"
                                        />
                                        <InputError
                                            className="mt-1"
                                            message={errors.sender_name}
                                        />
                                    </div>

                                    <div className="flex items-center gap-3">
                                        <Button disabled={processing}>
                                            {resend.connected
                                                ? 'Rotate key'
                                                : 'Connect Resend'}
                                        </Button>

                                        <Transition
                                            show={
                                                recentlySuccessful ||
                                                status === 'resend-connected' ||
                                                status === 'resend-rotated' ||
                                                status === 'resend-updated'
                                            }
                                            enter="transition ease-in-out"
                                            enterFrom="opacity-0"
                                            leave="transition ease-in-out"
                                            leaveTo="opacity-0"
                                        >
                                            <p className="text-sm text-zinc-600 dark:text-zinc-300">
                                                {status === 'resend-rotated'
                                                    ? 'Key rotated'
                                                    : status ===
                                                        'resend-updated'
                                                      ? 'Integration updated'
                                                      : 'Saved'}
                                            </p>
                                        </Transition>
                                    </div>
                                </>
                            )}
                        </Form>

                        <div className="mt-5 flex flex-wrap items-center gap-3">
                            <Form
                                action="/settings/integrations/resend/test"
                                method="post"
                                options={{ preserveScroll: true }}
                            >
                                {({ processing, errors }) => (
                                    <>
                                        <Button
                                            type="submit"
                                            variant="secondary"
                                            disabled={
                                                processing || !resend.connected
                                            }
                                            onClick={() => {
                                                capture(
                                                    'settings_integrations.resend_test_submitted',
                                                    {
                                                        resendConnected:
                                                            resend.connected,
                                                    },
                                                );
                                            }}
                                        >
                                            Test connection
                                        </Button>
                                        <InputError
                                            className="mt-1"
                                            message={errors.resend}
                                        />
                                    </>
                                )}
                            </Form>

                            <Form
                                action="/settings/integrations/resend"
                                method="delete"
                                options={{ preserveScroll: true }}
                            >
                                {({ processing }) => (
                                    <Button
                                        type="submit"
                                        variant="destructive"
                                        disabled={
                                            processing || !resend.connected
                                        }
                                        onClick={() => {
                                            capture(
                                                'settings_integrations.resend_disconnect_submitted',
                                                {
                                                    resendConnected:
                                                        resend.connected,
                                                },
                                            );
                                        }}
                                    >
                                        Disconnect
                                    </Button>
                                )}
                            </Form>

                            <Transition
                                show={
                                    status === 'resend-disconnected' ||
                                    status === 'resend-test-queued'
                                }
                                enter="transition ease-in-out"
                                enterFrom="opacity-0"
                                leave="transition ease-in-out"
                                leaveTo="opacity-0"
                            >
                                <p className="text-sm text-zinc-600 dark:text-zinc-300">
                                    {status === 'resend-disconnected'
                                        ? 'Resend disconnected'
                                        : 'Connection test queued'}
                                </p>
                            </Transition>
                        </div>

                        <div className="mt-6 border-t border-border pt-4">
                            <h3 className="mb-2 text-sm font-semibold">
                                Recent lifecycle events
                            </h3>
                            {resendAudit.length === 0 ? (
                                <p className="text-sm text-muted-foreground">
                                    No events recorded yet.
                                </p>
                            ) : (
                                <ul className="space-y-1 text-sm text-muted-foreground">
                                    {resendAudit.map((item, index) => (
                                        <li
                                            key={`${item.action}-${item.createdAt ?? index}`}
                                        >
                                            <span className="font-medium text-foreground">
                                                {item.action}
                                            </span>{' '}
                                            {item.createdAt ? (
                                                <span>
                                                    at{' '}
                                                    {new Date(
                                                        item.createdAt,
                                                    ).toLocaleString()}
                                                </span>
                                            ) : null}
                                        </li>
                                    ))}
                                </ul>
                            )}
                        </div>
                    </div>
                </div>
            </SettingsLayout>
        </>
    );
}
