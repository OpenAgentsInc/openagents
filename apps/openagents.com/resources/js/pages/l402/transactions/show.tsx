import { Head, Link } from '@inertiajs/react';
import { L402PageNav } from '@/components/l402/page-nav';
import AppLayout from '@/layouts/app-layout';
import type { BreadcrumbItem } from '@/types';

type Tx = {
    eventId: number;
    createdAt: string;
    threadId: string;
    threadTitle: string;
    runId: string;
    runStatus: string | null;
    runStartedAt: string | null;
    runCompletedAt: string | null;
    status: string;
    host: string;
    scope: string | null;
    paid: boolean;
    cacheHit: boolean;
    cacheStatus: string | null;
    amountMsats: number | null;
    amountSats: number | null;
    quotedAmountMsats: number | null;
    quotedAmountSats: number | null;
    maxSpendMsats: number | null;
    maxSpendSats: number | null;
    proofReference: string | null;
    denyCode: string | null;
    responseStatusCode: number | null;
    responseBodySha256: string | null;
    toolCallId: string | null;
    rawPayload: Record<string, unknown>;
};

type Props = {
    transaction: Tx;
};

function statusClass(status: string): string {
    if (status === 'completed') return 'bg-emerald-500/15 text-emerald-300 border-emerald-500/30';
    if (status === 'cached') return 'bg-sky-500/15 text-sky-300 border-sky-500/30';
    if (status === 'blocked') return 'bg-amber-500/15 text-amber-300 border-amber-500/30';
    if (status === 'failed') return 'bg-rose-500/15 text-rose-300 border-rose-500/30';
    return 'bg-zinc-500/15 text-zinc-300 border-zinc-500/30';
}

export default function L402TransactionDetailPage({ transaction }: Props) {
    const breadcrumbs: BreadcrumbItem[] = [
        { title: 'L402', href: '/l402' },
        { title: 'Transactions', href: '/l402/transactions' },
        { title: `Event ${transaction.eventId}`, href: `/l402/transactions/${transaction.eventId}` },
    ];

    return (
        <AppLayout breadcrumbs={breadcrumbs}>
            <Head title={`L402 Transaction #${transaction.eventId}`} />
            <div className="flex h-full flex-1 flex-col gap-4 overflow-x-auto rounded-xl p-4">
                <L402PageNav />

                <div className="rounded-xl border border-sidebar-border/70 bg-card p-4">
                    <div className="mb-3 flex items-start justify-between gap-2">
                        <div>
                            <h1 className="text-lg font-semibold">L402 Transaction #{transaction.eventId}</h1>
                            <p className="text-xs text-muted-foreground">{transaction.createdAt}</p>
                        </div>
                        <span className={`rounded border px-2 py-0.5 text-xs ${statusClass(transaction.status)}`}>
                            {transaction.status}
                        </span>
                    </div>

                    <div className="grid gap-4 md:grid-cols-2">
                        <div className="space-y-1 text-sm">
                            <div><span className="text-muted-foreground">host:</span> {transaction.host}</div>
                            <div><span className="text-muted-foreground">scope:</span> {transaction.scope ?? 'no-scope'}</div>
                            <div><span className="text-muted-foreground">amount:</span> {transaction.amountSats ?? transaction.quotedAmountSats ?? 0} sats</div>
                            <div><span className="text-muted-foreground">paid:</span> {transaction.paid ? 'true' : 'false'}</div>
                            <div><span className="text-muted-foreground">cache:</span> {transaction.cacheHit ? 'hit' : transaction.cacheStatus ?? 'miss'}</div>
                            <div><span className="text-muted-foreground">max spend:</span> {transaction.maxSpendSats ?? '-'} sats</div>
                            <div><span className="text-muted-foreground">http status:</span> {transaction.responseStatusCode ?? '-'}</div>
                        </div>

                        <div className="space-y-1 text-sm">
                            <div><span className="text-muted-foreground">thread:</span> {transaction.threadTitle}</div>
                            <div><span className="text-muted-foreground">run:</span> {transaction.runId}</div>
                            <div><span className="text-muted-foreground">run status:</span> {transaction.runStatus ?? '-'}</div>
                            <div><span className="text-muted-foreground">tool call:</span> {transaction.toolCallId ?? '-'}</div>
                            <div className="break-all"><span className="text-muted-foreground">proof:</span> {transaction.proofReference ?? '-'}</div>
                            <div><span className="text-muted-foreground">deny code:</span> {transaction.denyCode ?? '-'}</div>
                            <div className="break-all"><span className="text-muted-foreground">body sha256:</span> {transaction.responseBodySha256 ?? '-'}</div>
                        </div>
                    </div>

                    <div className="mt-4 flex gap-2">
                        <Link href="/l402/transactions" className="text-xs text-primary hover:underline">
                            Back to transactions
                        </Link>
                        <Link href={`/chat/${transaction.threadId}`} className="text-xs text-primary hover:underline">
                            Open conversation
                        </Link>
                    </div>
                </div>

                <div className="rounded-xl border border-sidebar-border/70 bg-card p-4">
                    <div className="mb-2 text-xs uppercase tracking-wide text-muted-foreground">Raw receipt payload</div>
                    <pre className="overflow-x-auto rounded-md bg-muted/40 p-3 text-xs leading-relaxed">
                        {JSON.stringify(transaction.rawPayload, null, 2)}
                    </pre>
                </div>
            </div>
        </AppLayout>
    );
}
