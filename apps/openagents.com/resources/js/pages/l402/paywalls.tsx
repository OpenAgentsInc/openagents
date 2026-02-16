import { Head } from '@inertiajs/react';
import { L402PageNav } from '@/components/l402/page-nav';
import AppLayout from '@/layouts/app-layout';
import type { BreadcrumbItem } from '@/types';

type Paywall = {
    host: string;
    scope: string;
    attempts: number;
    paid: number;
    cached: number;
    blocked: number;
    failed: number;
    totalPaidSats: number | null;
    lastAttemptAt: string | null;
    lastStatus: string;
};

type Props = {
    paywalls: Paywall[];
    summary: {
        uniqueTargets: number;
        totalAttempts: number;
        totalPaidCount: number;
    };
};

const breadcrumbs: BreadcrumbItem[] = [
    { title: 'L402', href: '/l402' },
    { title: 'Paywalls', href: '/l402/paywalls' },
];

function statusClass(status: string): string {
    if (status === 'completed') return 'bg-emerald-500/15 text-emerald-300 border-emerald-500/30';
    if (status === 'cached') return 'bg-sky-500/15 text-sky-300 border-sky-500/30';
    if (status === 'blocked') return 'bg-amber-500/15 text-amber-300 border-amber-500/30';
    if (status === 'failed') return 'bg-rose-500/15 text-rose-300 border-rose-500/30';
    return 'bg-zinc-500/15 text-zinc-300 border-zinc-500/30';
}

export default function L402PaywallsPage({ paywalls, summary }: Props) {
    return (
        <AppLayout breadcrumbs={breadcrumbs}>
            <Head title="L402 Paywalls" />
            <div className="flex h-full flex-1 flex-col gap-4 overflow-x-auto rounded-xl p-4">
                <L402PageNav />

                <div className="grid gap-3 md:grid-cols-3">
                    <div className="rounded-xl border border-sidebar-border/70 bg-card p-4">
                        <div className="text-xs uppercase tracking-wide text-muted-foreground">Unique targets</div>
                        <div className="mt-1 text-2xl font-semibold">{summary.uniqueTargets}</div>
                    </div>
                    <div className="rounded-xl border border-sidebar-border/70 bg-card p-4">
                        <div className="text-xs uppercase tracking-wide text-muted-foreground">Total attempts</div>
                        <div className="mt-1 text-2xl font-semibold">{summary.totalAttempts}</div>
                    </div>
                    <div className="rounded-xl border border-sidebar-border/70 bg-card p-4">
                        <div className="text-xs uppercase tracking-wide text-muted-foreground">Paid attempts</div>
                        <div className="mt-1 text-2xl font-semibold">{summary.totalPaidCount}</div>
                    </div>
                </div>

                <div className="rounded-xl border border-sidebar-border/70 bg-card p-4">
                    <div className="mb-3 text-xs uppercase tracking-wide text-muted-foreground">Observed paywall targets</div>
                    {paywalls.length === 0 ? (
                        <div className="text-sm text-muted-foreground">No L402 paywall targets observed yet.</div>
                    ) : (
                        <div className="overflow-x-auto">
                            <table className="w-full min-w-[960px] text-sm">
                                <thead className="text-left text-xs uppercase tracking-wide text-muted-foreground">
                                    <tr>
                                        <th className="px-2 py-2">Host / scope</th>
                                        <th className="px-2 py-2">Attempts</th>
                                        <th className="px-2 py-2">Paid</th>
                                        <th className="px-2 py-2">Cached</th>
                                        <th className="px-2 py-2">Blocked</th>
                                        <th className="px-2 py-2">Failed</th>
                                        <th className="px-2 py-2">Settled sats</th>
                                        <th className="px-2 py-2">Last status</th>
                                    </tr>
                                </thead>
                                <tbody>
                                    {paywalls.map((row) => (
                                        <tr key={`${row.host}-${row.scope}`} className="border-t border-sidebar-border/60">
                                            <td className="px-2 py-2 align-top">
                                                <div className="font-medium">{row.host}</div>
                                                <div className="text-xs text-muted-foreground">{row.scope || 'no-scope'}</div>
                                            </td>
                                            <td className="px-2 py-2 align-top">{row.attempts}</td>
                                            <td className="px-2 py-2 align-top">{row.paid}</td>
                                            <td className="px-2 py-2 align-top">{row.cached}</td>
                                            <td className="px-2 py-2 align-top">{row.blocked}</td>
                                            <td className="px-2 py-2 align-top">{row.failed}</td>
                                            <td className="px-2 py-2 align-top">{row.totalPaidSats ?? 0}</td>
                                            <td className="px-2 py-2 align-top">
                                                <span className={`rounded border px-2 py-0.5 text-xs ${statusClass(row.lastStatus)}`}>
                                                    {row.lastStatus}
                                                </span>
                                            </td>
                                        </tr>
                                    ))}
                                </tbody>
                            </table>
                        </div>
                    )}
                </div>
            </div>
        </AppLayout>
    );
}
