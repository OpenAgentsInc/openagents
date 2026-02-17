import { Head } from '@inertiajs/react';
import { L402PageNav } from '@/components/l402/page-nav';
import AppLayout from '@/layouts/app-layout';
import type { BreadcrumbItem } from '@/types';

type DeploymentEvent = {
    eventId: number;
    type: string;
    createdAt: string;
    payload: Record<string, unknown>;
};

type Props = {
    deployments: DeploymentEvent[];
    configSnapshot: {
        enforceHostAllowlist: boolean;
        allowlistHosts: string[];
        invoicePayer: string;
        credentialTtlSeconds: number;
        paymentTimeoutMs: number;
        demoPresets: string[];
    };
};

const breadcrumbs: BreadcrumbItem[] = [
    { title: 'L402', href: '/l402' },
    { title: 'Deployments', href: '/l402/deployments' },
];

function pretty(value: unknown): string {
    try {
        return JSON.stringify(value, null, 2);
    } catch {
        return String(value);
    }
}

export default function L402DeploymentsPage({ deployments, configSnapshot }: Props) {
    return (
        <AppLayout breadcrumbs={breadcrumbs}>
            <Head title="L402 Deployments" />
            <div className="flex h-full flex-1 flex-col gap-4 overflow-x-auto rounded-xl p-4">
                <L402PageNav />

                <div className="rounded-xl border border-sidebar-border/70 bg-card p-4">
                    <div className="mb-3 text-xs uppercase tracking-wide text-muted-foreground">Gateway / executor config</div>
                    <div className="grid gap-3 md:grid-cols-2">
                        <div className="space-y-1 text-sm">
                            <div>
                                invoice payer: <span className="font-mono">{configSnapshot.invoicePayer}</span>
                            </div>
                            <div>
                                credential ttl: <span className="font-mono">{configSnapshot.credentialTtlSeconds}s</span>
                            </div>
                            <div>
                                payment timeout: <span className="font-mono">{configSnapshot.paymentTimeoutMs}ms</span>
                            </div>
                        </div>
                        <div className="space-y-1 text-sm">
                            <div>
                                allowlist:{' '}
                                <span className="font-mono">
                                    {configSnapshot.enforceHostAllowlist
                                        ? configSnapshot.allowlistHosts.length > 0
                                            ? configSnapshot.allowlistHosts.join(', ')
                                            : '(enabled, but empty)'
                                        : 'disabled (all domains allowed)'}
                                </span>
                            </div>
                            <div>
                                demo presets:{' '}
                                <span className="font-mono">
                                    {configSnapshot.demoPresets.length > 0
                                        ? configSnapshot.demoPresets.join(', ')
                                        : '(none)'}
                                </span>
                            </div>
                        </div>
                    </div>
                </div>

                <div className="rounded-xl border border-sidebar-border/70 bg-card p-4">
                    <div className="mb-3 text-xs uppercase tracking-wide text-muted-foreground">Deployment and gateway events</div>
                    {deployments.length === 0 ? (
                        <div className="text-sm text-muted-foreground">No gateway deployment events captured yet.</div>
                    ) : (
                        <div className="space-y-3">
                            {deployments.map((event) => (
                                <div key={event.eventId} className="rounded border border-sidebar-border/60 p-3">
                                    <div className="flex flex-wrap items-center justify-between gap-2">
                                        <div className="font-medium">{event.type}</div>
                                        <div className="text-xs text-muted-foreground">{event.createdAt}</div>
                                    </div>
                                    <div className="mt-1 text-xs text-muted-foreground">event #{event.eventId}</div>
                                    <pre className="mt-2 overflow-x-auto rounded bg-muted/40 p-2 text-xs leading-relaxed">
                                        {pretty(event.payload)}
                                    </pre>
                                </div>
                            ))}
                        </div>
                    )}
                </div>
            </div>
        </AppLayout>
    );
}
