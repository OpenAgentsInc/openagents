import { Head, Link } from '@inertiajs/react';
import { useEffect } from 'react';
import { L402PageNav } from '@/components/l402/page-nav';
import { usePostHogEvent } from '@/hooks/use-posthog-event';

type L402Receipt = {
    eventId: number;
    status: string;
    host: string;
    scope: string | null;
    paid: boolean;
    cacheHit: boolean;
    cacheStatus: string | null;
    amountSats: number | null;
    quotedAmountSats: number | null;
    proofReference: string | null;
    denyCode: string | null;
    createdAt: string;
};

type Props = {
    summary: {
        totalAttempts: number;
        paidCount: number;
        cachedCount: number;
        blockedCount: number;
        failedCount: number;
        totalPaidMsats: number;
        totalPaidSats: number | null;
    };
    lastPaid: L402Receipt | null;
    recent: L402Receipt[];
    settings: {
        enforceHostAllowlist: boolean;
        allowlistHosts: string[];
        invoicePayer: string;
        credentialTtlSeconds: number;
        paymentTimeoutMs: number;
        responseMaxBytes: number;
        responsePreviewBytes: number;
    };
};

function statusClass(status: string): string {
    if (status === 'completed') return 'bg-emerald-500/15 text-emerald-300 border-emerald-500/30';
    if (status === 'cached') return 'bg-sky-500/15 text-sky-300 border-sky-500/30';
    if (status === 'blocked') return 'bg-amber-500/15 text-amber-300 border-amber-500/30';
    if (status === 'failed') return 'bg-rose-500/15 text-rose-300 border-rose-500/30';
    return 'bg-zinc-500/15 text-zinc-300 border-zinc-500/30';
}

export default function L402WalletPage({ summary, lastPaid, recent, settings }: Props) {
    const capture = usePostHogEvent('l402');

    useEffect(() => {
        capture('l402.wallet_page_opened', {
            paidCount: summary.paidCount,
            recentCount: recent.length,
            hasLastPaid: Boolean(lastPaid),
            invoicePayer: settings.invoicePayer,
        });
    }, [capture, lastPaid, recent.length, settings.invoicePayer, summary.paidCount]);

    return (
        <>
            <Head title="L402 Wallet" />
            <div className="flex h-full flex-1 flex-col gap-4 overflow-x-auto rounded-xl p-4">
                <L402PageNav />

                <div className="grid gap-3 md:grid-cols-3">
                    <div className="rounded-xl border border-sidebar-border/70 bg-card p-4">
                        <div className="text-xs uppercase tracking-wide text-muted-foreground">Total attempts</div>
                        <div className="mt-1 text-2xl font-semibold">{summary.totalAttempts}</div>
                    </div>
                    <div className="rounded-xl border border-sidebar-border/70 bg-card p-4">
                        <div className="text-xs uppercase tracking-wide text-muted-foreground">Paid + cached</div>
                        <div className="mt-1 text-2xl font-semibold">{summary.paidCount + summary.cachedCount}</div>
                        <div className="mt-1 text-xs text-muted-foreground">
                            paid {summary.paidCount} · cached {summary.cachedCount}
                        </div>
                    </div>
                    <div className="rounded-xl border border-sidebar-border/70 bg-card p-4">
                        <div className="text-xs uppercase tracking-wide text-muted-foreground">Total settled</div>
                        <div className="mt-1 text-2xl font-semibold">{summary.totalPaidSats ?? 0} sats</div>
                        <div className="mt-1 text-xs text-muted-foreground">{summary.totalPaidMsats} msats</div>
                    </div>
                </div>

                <div className="grid gap-4 lg:grid-cols-2">
                    <div className="rounded-xl border border-sidebar-border/70 bg-card p-4">
                        <div className="text-xs uppercase tracking-wide text-muted-foreground">Last paid transaction</div>
                        {lastPaid ? (
                            <div className="mt-3 space-y-2 text-sm">
                                <div className="flex items-center justify-between gap-2">
                                    <div className="font-medium">{lastPaid.host}</div>
                                    <span className={`rounded border px-2 py-0.5 text-xs ${statusClass(lastPaid.status)}`}>
                                        {lastPaid.status}
                                    </span>
                                </div>
                                <div className="text-muted-foreground">
                                    {lastPaid.amountSats ?? lastPaid.quotedAmountSats ?? 0} sats
                                    {lastPaid.scope ? ` · ${lastPaid.scope}` : ''}
                                </div>
                                {lastPaid.proofReference ? (
                                    <div className="font-mono text-xs text-muted-foreground">
                                        {lastPaid.proofReference}
                                    </div>
                                ) : null}
                                <Link
                                    href={`/l402/transactions/${lastPaid.eventId}`}
                                    onClick={() => {
                                        capture('l402.transaction_detail_clicked', {
                                            source: 'wallet_last_paid',
                                            eventId: lastPaid.eventId,
                                        });
                                    }}
                                    className="inline-flex text-xs text-primary hover:underline"
                                >
                                    Open transaction detail
                                </Link>
                            </div>
                        ) : (
                            <div className="mt-3 text-sm text-muted-foreground">No paid L402 transactions yet.</div>
                        )}
                    </div>

                    <div className="rounded-xl border border-sidebar-border/70 bg-card p-4">
                        <div className="text-xs uppercase tracking-wide text-muted-foreground">Runtime settings</div>
                        <div className="mt-3 space-y-1 text-sm">
                            <div>invoice payer: <span className="font-mono">{settings.invoicePayer}</span></div>
                            <div>payment timeout: <span className="font-mono">{settings.paymentTimeoutMs}ms</span></div>
                            <div>credential ttl: <span className="font-mono">{settings.credentialTtlSeconds}s</span></div>
                            <div>response max bytes: <span className="font-mono">{settings.responseMaxBytes}</span></div>
                            <div>response preview bytes: <span className="font-mono">{settings.responsePreviewBytes}</span></div>
                        </div>
                        <div className="mt-3 text-xs text-muted-foreground">
                            host allowlist: {' '}
                            {settings.enforceHostAllowlist
                                ? settings.allowlistHosts.length > 0
                                    ? settings.allowlistHosts.join(', ')
                                    : '(enabled, but empty)'
                                : 'disabled (all domains allowed)'}
                        </div>
                    </div>
                </div>

                <div className="rounded-xl border border-sidebar-border/70 bg-card p-4">
                    <div className="mb-3 flex items-center justify-between gap-2">
                        <div className="text-xs uppercase tracking-wide text-muted-foreground">Recent L402 attempts</div>
                        <Link
                            href="/l402/transactions"
                            onClick={() => {
                                capture('l402.transactions_page_clicked', {
                                    source: 'wallet_recent_header',
                                });
                            }}
                            className="text-xs text-primary hover:underline"
                        >
                            View all
                        </Link>
                    </div>
                    <div className="space-y-2">
                        {recent.length === 0 ? (
                            <div className="text-sm text-muted-foreground">No L402 attempts recorded.</div>
                        ) : (
                            recent.slice(0, 10).map((item) => (
                                <Link
                                    key={item.eventId}
                                    href={`/l402/transactions/${item.eventId}`}
                                    onClick={() => {
                                        capture('l402.transaction_detail_clicked', {
                                            source: 'wallet_recent_list',
                                            eventId: item.eventId,
                                            status: item.status,
                                        });
                                    }}
                                    className="flex items-center justify-between gap-2 rounded border border-sidebar-border/60 px-3 py-2 text-sm hover:bg-muted/50"
                                >
                                    <div className="min-w-0">
                                        <div className="truncate font-medium">{item.host}</div>
                                        <div className="truncate text-xs text-muted-foreground">
                                            {item.amountSats ?? item.quotedAmountSats ?? 0} sats
                                            {item.scope ? ` · ${item.scope}` : ''}
                                        </div>
                                    </div>
                                    <span className={`shrink-0 rounded border px-2 py-0.5 text-xs ${statusClass(item.status)}`}>
                                        {item.status}
                                    </span>
                                </Link>
                            ))
                        )}
                    </div>
                </div>
            </div>
        </>
    );
}
