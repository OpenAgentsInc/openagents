import { Head, Link } from '@inertiajs/react';
import { L402PageNav } from '@/components/l402/page-nav';
import AppLayout from '@/layouts/app-layout';
import type { BreadcrumbItem } from '@/types';

type Settlement = {
    eventId: number;
    createdAt: string;
    host: string;
    scope: string | null;
    amountSats: number | null;
    amountMsats: number | null;
    proofReference: string | null;
    threadTitle: string;
    status: string;
};

type Props = {
    summary: {
        settledCount: number;
        totalMsats: number;
        totalSats: number | null;
        latestSettlementAt: string | null;
    };
    daily: Array<{
        date: string;
        count: number;
        totalMsats: number;
        totalSats: number | null;
    }>;
    settlements: Settlement[];
};

const breadcrumbs: BreadcrumbItem[] = [
    { title: 'L402', href: '/l402' },
    { title: 'Settlements', href: '/l402/settlements' },
];

export default function L402SettlementsPage({ summary, daily, settlements }: Props) {
    return (
        <AppLayout breadcrumbs={breadcrumbs}>
            <Head title="L402 Settlements" />
            <div className="flex h-full flex-1 flex-col gap-4 overflow-x-auto rounded-xl p-4">
                <L402PageNav />

                <div className="grid gap-3 md:grid-cols-3">
                    <div className="rounded-xl border border-sidebar-border/70 bg-card p-4">
                        <div className="text-xs uppercase tracking-wide text-muted-foreground">Settled payments</div>
                        <div className="mt-1 text-2xl font-semibold">{summary.settledCount}</div>
                    </div>
                    <div className="rounded-xl border border-sidebar-border/70 bg-card p-4">
                        <div className="text-xs uppercase tracking-wide text-muted-foreground">Total settled sats</div>
                        <div className="mt-1 text-2xl font-semibold">{summary.totalSats ?? 0}</div>
                        <div className="mt-1 text-xs text-muted-foreground">{summary.totalMsats} msats</div>
                    </div>
                    <div className="rounded-xl border border-sidebar-border/70 bg-card p-4">
                        <div className="text-xs uppercase tracking-wide text-muted-foreground">Latest settlement</div>
                        <div className="mt-1 text-sm font-medium">{summary.latestSettlementAt ?? 'n/a'}</div>
                    </div>
                </div>

                <div className="rounded-xl border border-sidebar-border/70 bg-card p-4">
                    <div className="mb-3 text-xs uppercase tracking-wide text-muted-foreground">Daily totals</div>
                    {daily.length === 0 ? (
                        <div className="text-sm text-muted-foreground">No settlements yet.</div>
                    ) : (
                        <div className="overflow-x-auto">
                            <table className="w-full min-w-[640px] text-sm">
                                <thead className="text-left text-xs uppercase tracking-wide text-muted-foreground">
                                    <tr>
                                        <th className="px-2 py-2">Date</th>
                                        <th className="px-2 py-2">Count</th>
                                        <th className="px-2 py-2">Total sats</th>
                                        <th className="px-2 py-2">Total msats</th>
                                    </tr>
                                </thead>
                                <tbody>
                                    {daily.map((row) => (
                                        <tr key={row.date} className="border-t border-sidebar-border/60">
                                            <td className="px-2 py-2 align-top">{row.date || 'unknown'}</td>
                                            <td className="px-2 py-2 align-top">{row.count}</td>
                                            <td className="px-2 py-2 align-top">{row.totalSats ?? 0}</td>
                                            <td className="px-2 py-2 align-top">{row.totalMsats}</td>
                                        </tr>
                                    ))}
                                </tbody>
                            </table>
                        </div>
                    )}
                </div>

                <div className="rounded-xl border border-sidebar-border/70 bg-card p-4">
                    <div className="mb-3 text-xs uppercase tracking-wide text-muted-foreground">Recent settled transactions</div>
                    {settlements.length === 0 ? (
                        <div className="text-sm text-muted-foreground">No settled transactions yet.</div>
                    ) : (
                        <div className="space-y-2">
                            {settlements.map((item) => (
                                <div
                                    key={item.eventId}
                                    className="rounded border border-sidebar-border/60 px-3 py-2 text-sm"
                                >
                                    <div className="flex flex-wrap items-center justify-between gap-2">
                                        <div className="font-medium">{item.host}</div>
                                        <div className="text-xs text-muted-foreground">{item.createdAt}</div>
                                    </div>
                                    <div className="mt-1 text-xs text-muted-foreground">
                                        {item.amountSats ?? 0} sats
                                        {item.scope ? ` · ${item.scope}` : ''}
                                        {item.proofReference ? ` · ${item.proofReference}` : ''}
                                    </div>
                                    <div className="mt-1 flex flex-wrap gap-3 text-xs text-muted-foreground">
                                        <span>{item.threadTitle}</span>
                                        <span>status: {item.status}</span>
                                        <Link href={`/l402/transactions/${item.eventId}`} className="text-primary hover:underline">
                                            View detail
                                        </Link>
                                    </div>
                                </div>
                            ))}
                        </div>
                    )}
                </div>
            </div>
        </AppLayout>
    );
}
