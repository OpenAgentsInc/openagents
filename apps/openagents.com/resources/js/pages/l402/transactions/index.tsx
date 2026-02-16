import { Head, Link } from '@inertiajs/react';
import { L402PageNav } from '@/components/l402/page-nav';
import AppLayout from '@/layouts/app-layout';
import type { BreadcrumbItem } from '@/types';

type Tx = {
    eventId: number;
    createdAt: string;
    status: string;
    host: string;
    scope: string | null;
    paid: boolean;
    cacheStatus: string | null;
    amountSats: number | null;
    quotedAmountSats: number | null;
    proofReference: string | null;
    denyCode: string | null;
    responseStatusCode: number | null;
    threadTitle: string;
};

type Props = {
    transactions: Tx[];
    pagination: {
        currentPage: number;
        lastPage: number;
        perPage: number;
        total: number;
        hasMorePages: boolean;
    };
};

const breadcrumbs: BreadcrumbItem[] = [
    { title: 'L402', href: '/l402' },
    { title: 'Transactions', href: '/l402/transactions' },
];

function statusClass(status: string): string {
    if (status === 'completed') return 'bg-emerald-500/15 text-emerald-300 border-emerald-500/30';
    if (status === 'cached') return 'bg-sky-500/15 text-sky-300 border-sky-500/30';
    if (status === 'blocked') return 'bg-amber-500/15 text-amber-300 border-amber-500/30';
    if (status === 'failed') return 'bg-rose-500/15 text-rose-300 border-rose-500/30';
    return 'bg-zinc-500/15 text-zinc-300 border-zinc-500/30';
}

export default function L402TransactionsPage({ transactions, pagination }: Props) {
    return (
        <AppLayout breadcrumbs={breadcrumbs}>
            <Head title="L402 Transactions" />
            <div className="flex h-full flex-1 flex-col gap-4 overflow-x-auto rounded-xl p-4">
                <L402PageNav />

                <div className="rounded-xl border border-sidebar-border/70 bg-card p-4">
                    <div className="mb-3 flex items-center justify-between">
                        <div>
                            <h1 className="text-lg font-semibold">L402 Transactions</h1>
                            <p className="text-xs text-muted-foreground">
                                {pagination.total} total · page {pagination.currentPage} of {pagination.lastPage}
                            </p>
                        </div>
                    </div>

                    {transactions.length === 0 ? (
                        <div className="text-sm text-muted-foreground">No L402 transactions recorded yet.</div>
                    ) : (
                        <div className="overflow-x-auto">
                            <table className="w-full min-w-[980px] text-sm">
                                <thead className="text-left text-xs uppercase tracking-wide text-muted-foreground">
                                    <tr>
                                        <th className="px-2 py-2">Status</th>
                                        <th className="px-2 py-2">Host</th>
                                        <th className="px-2 py-2">Amount</th>
                                        <th className="px-2 py-2">Context</th>
                                        <th className="px-2 py-2">Proof / policy</th>
                                        <th className="px-2 py-2">HTTP</th>
                                        <th className="px-2 py-2">At</th>
                                    </tr>
                                </thead>
                                <tbody>
                                    {transactions.map((tx) => (
                                        <tr key={tx.eventId} className="border-t border-sidebar-border/60">
                                            <td className="px-2 py-2 align-top">
                                                <span className={`rounded border px-2 py-0.5 text-xs ${statusClass(tx.status)}`}>
                                                    {tx.status}
                                                </span>
                                            </td>
                                            <td className="px-2 py-2 align-top">
                                                <Link href={`/l402/transactions/${tx.eventId}`} className="font-medium hover:underline">
                                                    {tx.host}
                                                </Link>
                                                <div className="text-xs text-muted-foreground">{tx.scope ?? 'no-scope'}</div>
                                            </td>
                                            <td className="px-2 py-2 align-top">
                                                {tx.amountSats ?? tx.quotedAmountSats ?? 0} sats
                                                <div className="text-xs text-muted-foreground">
                                                    {tx.paid ? 'paid' : tx.cacheStatus === 'hit' ? 'cached' : 'not paid'}
                                                </div>
                                            </td>
                                            <td className="px-2 py-2 align-top">
                                                <div className="truncate max-w-[240px]">{tx.threadTitle}</div>
                                            </td>
                                            <td className="px-2 py-2 align-top">
                                                <div className="max-w-[260px] truncate font-mono text-xs text-muted-foreground">
                                                    {tx.proofReference ?? tx.denyCode ?? '-'}
                                                </div>
                                            </td>
                                            <td className="px-2 py-2 align-top">{tx.responseStatusCode ?? '-'}</td>
                                            <td className="px-2 py-2 align-top">
                                                <span className="text-xs text-muted-foreground">{tx.createdAt}</span>
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
