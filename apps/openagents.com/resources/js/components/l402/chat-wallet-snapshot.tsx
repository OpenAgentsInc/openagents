import { useCallback, useEffect, useState } from 'react';
import { Button } from '@/components/ui/button';
import {
    Card,
    CardAction,
    CardContent,
    CardDescription,
    CardHeader,
    CardTitle,
} from '@/components/ui/card';

type WalletSnapshot = {
    sparkBalanceSats: number | null;
    sparkStatus: string | null;
    totalPaidSats: number | null;
    paidCount: number;
    cachedCount: number;
    blockedCount: number;
    invoicePayer: string;
    lastProofReference: string | null;
    lastUpdatedAt: string | null;
};

type Props = {
    refreshKey: string;
    disabled?: boolean;
    variant?: 'chat' | 'sidebar';
};

function toNullableNumber(value: unknown): number | null {
    if (typeof value === 'number' && Number.isFinite(value)) {
        return value;
    }

    if (typeof value === 'string' && value.trim() !== '') {
        const parsed = Number(value);

        return Number.isFinite(parsed) ? parsed : null;
    }

    return null;
}

function toNullableString(value: unknown): string | null {
    if (typeof value !== 'string') {
        return null;
    }

    const trimmed = value.trim();

    return trimmed === '' ? null : trimmed;
}

function compactProofReference(value: string | null): string | null {
    if (!value) {
        return null;
    }

    if (value.length <= 28) {
        return value;
    }

    return `${value.slice(0, 24)}...`;
}

function formatSats(value: number | null): string {
    if (value === null || !Number.isFinite(value)) {
        return 'n/a';
    }

    const rounded = Math.round(value);

    return `${rounded.toLocaleString()} sats`;
}

function buildSnapshot(payload: unknown): WalletSnapshot {
    const root = typeof payload === 'object' && payload !== null
        ? payload as Record<string, unknown>
        : {};

    const data = typeof root.data === 'object' && root.data !== null
        ? root.data as Record<string, unknown>
        : {};

    const summary = typeof data.summary === 'object' && data.summary !== null
        ? data.summary as Record<string, unknown>
        : {};

    const sparkWallet = typeof data.sparkWallet === 'object' && data.sparkWallet !== null
        ? data.sparkWallet as Record<string, unknown>
        : {};

    const lastPaid = typeof data.lastPaid === 'object' && data.lastPaid !== null
        ? data.lastPaid as Record<string, unknown>
        : {};

    const settings = typeof data.settings === 'object' && data.settings !== null
        ? data.settings as Record<string, unknown>
        : {};

    return {
        sparkBalanceSats: toNullableNumber(sparkWallet.balanceSats),
        sparkStatus: toNullableString(sparkWallet.status),
        totalPaidSats: toNullableNumber(summary.totalPaidSats),
        paidCount: Math.max(0, Math.trunc(toNullableNumber(summary.paidCount) ?? 0)),
        cachedCount: Math.max(0, Math.trunc(toNullableNumber(summary.cachedCount) ?? 0)),
        blockedCount: Math.max(0, Math.trunc(toNullableNumber(summary.blockedCount) ?? 0)),
        invoicePayer: toNullableString(settings.invoicePayer) ?? 'unknown',
        lastProofReference: compactProofReference(toNullableString(lastPaid.proofReference)),
        lastUpdatedAt: toNullableString(sparkWallet.lastSyncedAt),
    };
}

async function fetchWalletSnapshot(signal?: AbortSignal): Promise<WalletSnapshot> {
    const response = await fetch('/api/l402/wallet', {
        credentials: 'include',
        headers: {
            Accept: 'application/json',
        },
        signal,
    });

    if (!response.ok) {
        throw new Error(`Wallet snapshot request failed (${response.status})`);
    }

    const json = await response.json();

    return buildSnapshot(json);
}

export function ChatWalletSnapshot({ refreshKey, disabled = false, variant = 'chat' }: Props) {
    const [snapshot, setSnapshot] = useState<WalletSnapshot | null>(null);
    const [loading, setLoading] = useState(false);
    const [error, setError] = useState<string | null>(null);

    const isSidebar = variant === 'sidebar';

    const refresh = useCallback(async (signal?: AbortSignal) => {
        setLoading(true);

        try {
            const nextSnapshot = await fetchWalletSnapshot(signal);
            setSnapshot(nextSnapshot);
            setError(null);
        } catch (err) {
            if (signal?.aborted) {
                return;
            }

            const message = err instanceof Error ? err.message : 'Failed to load wallet snapshot.';
            setError(message);
        } finally {
            if (!signal?.aborted) {
                setLoading(false);
            }
        }
    }, []);

    useEffect(() => {
        const controller = new AbortController();

        void refresh(controller.signal);

        return () => controller.abort();
    }, [refresh, refreshKey]);

    return (
        <Card className={isSidebar ? 'w-full gap-2 py-2' : 'mx-auto mb-3 w-full max-w-[768px] gap-3 py-3'}>
            <CardHeader className={isSidebar ? 'gap-1 px-3 py-2' : 'gap-1 px-4'}>
                <CardTitle className={isSidebar ? 'text-xs' : 'text-sm'}>Lightning wallet snapshot</CardTitle>
                <CardDescription className={isSidebar ? 'text-[11px]' : 'text-xs'}>
                    {isSidebar ? 'Spark balance and recent L402 receipt summary.' : 'Live Spark balance and recent L402 receipt summary.'}
                </CardDescription>
                <CardAction>
                    <Button
                        type="button"
                        variant="outline"
                        size="sm"
                        disabled={loading || disabled}
                        onClick={() => {
                            void refresh();
                        }}
                    >
                        {loading ? 'Refreshing...' : 'Refresh'}
                    </Button>
                </CardAction>
            </CardHeader>
            <CardContent className={isSidebar ? 'px-3 py-0' : 'px-4'}>
                {error ? (
                    <div className="text-xs text-destructive">{error}</div>
                ) : (
                    <div className={isSidebar ? 'grid gap-1 text-[11px] text-muted-foreground' : 'grid gap-1 text-xs text-muted-foreground sm:grid-cols-2'}>
                        <div>
                            <span className="font-medium text-foreground">Balance:</span>{' '}
                            {formatSats(snapshot?.sparkBalanceSats ?? null)}
                            {snapshot?.sparkStatus ? ` (${snapshot.sparkStatus})` : ''}
                        </div>
                        <div>
                            <span className="font-medium text-foreground">Paid:</span>{' '}
                            {snapshot ? `${snapshot.paidCount} (${formatSats(snapshot.totalPaidSats)})` : 'n/a'}
                        </div>
                        <div>
                            <span className="font-medium text-foreground">Cached / blocked:</span>{' '}
                            {snapshot ? `${snapshot.cachedCount} / ${snapshot.blockedCount}` : 'n/a'}
                        </div>
                        <div>
                            <span className="font-medium text-foreground">Payer:</span>{' '}
                            {snapshot?.invoicePayer ?? 'unknown'}
                        </div>
                        <div className={isSidebar ? '' : 'sm:col-span-2'}>
                            <span className="font-medium text-foreground">Last proof:</span>{' '}
                            {snapshot?.lastProofReference ?? 'none'}
                        </div>
                        <div className={isSidebar ? '' : 'sm:col-span-2'}>
                            <span className="font-medium text-foreground">Updated:</span>{' '}
                            {snapshot?.lastUpdatedAt ?? 'n/a'}
                        </div>
                    </div>
                )}
            </CardContent>
        </Card>
    );
}
