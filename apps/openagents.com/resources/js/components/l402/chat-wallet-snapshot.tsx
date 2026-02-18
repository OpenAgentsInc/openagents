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
import { usePostHogEvent } from '@/hooks/use-posthog-event';

type WalletSnapshot = {
    hasWallet: boolean;
    sparkBalanceSats: number | null;
    sparkStatus: string | null;
    sparkLastError: string | null;
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

    const sparkWalletRaw = typeof data.sparkWallet === 'object' && data.sparkWallet !== null
        ? data.sparkWallet as Record<string, unknown>
        : null;

    const sparkWallet = sparkWalletRaw ?? {};

    const lastPaid = typeof data.lastPaid === 'object' && data.lastPaid !== null
        ? data.lastPaid as Record<string, unknown>
        : {};

    const settings = typeof data.settings === 'object' && data.settings !== null
        ? data.settings as Record<string, unknown>
        : {};

    return {
        hasWallet: sparkWalletRaw !== null,
        sparkBalanceSats: toNullableNumber(sparkWallet.balanceSats),
        sparkStatus: toNullableString(sparkWallet.status),
        sparkLastError: toNullableString(sparkWallet.lastError),
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
    const capture = usePostHogEvent('l402');

    const isSidebar = variant === 'sidebar';

    const refresh = useCallback(async (signal?: AbortSignal) => {
        setLoading(true);

        try {
            const nextSnapshot = await fetchWalletSnapshot(signal);
            setSnapshot(nextSnapshot);
            setError(null);
            capture('l402.wallet_snapshot_loaded', {
                variant,
                hasWallet: nextSnapshot.hasWallet,
                sparkStatus: nextSnapshot.sparkStatus,
                hasWalletError: Boolean(nextSnapshot.sparkLastError),
            });
        } catch (err) {
            if (signal?.aborted) {
                return;
            }

            const message = err instanceof Error ? err.message : 'Failed to load wallet snapshot.';
            setError(message);
            capture('l402.wallet_snapshot_failed', {
                variant,
                errorMessage: message,
            });
        } finally {
            if (!signal?.aborted) {
                setLoading(false);
            }
        }
    }, [capture, variant]);

    useEffect(() => {
        const controller = new AbortController();

        void refresh(controller.signal);

        return () => controller.abort();
    }, [refresh, refreshKey]);

    if (isSidebar) {
        return (
            <Card className="w-full gap-1 py-2">
                <CardHeader className="gap-1 px-3 py-2">
                    <CardTitle className="text-xs">Lightning balance</CardTitle>
                    <CardAction>
                        <Button
                            type="button"
                            variant="outline"
                            size="sm"
                            disabled={loading || disabled}
                            onClick={() => {
                                capture('l402.wallet_snapshot_refresh_clicked', {
                                    variant,
                                    disabled,
                                });
                                void refresh();
                            }}
                        >
                            {loading ? '...' : 'Refresh'}
                        </Button>
                    </CardAction>
                </CardHeader>
                <CardContent className="px-3 py-0">
                    {error ? (
                        <div className="text-[11px] text-destructive">{error}</div>
                    ) : !snapshot?.hasWallet ? (
                        <div className="text-xs text-muted-foreground">No wallet yet</div>
                    ) : (
                        <div className="text-xs text-muted-foreground">
                            <span className="font-medium text-foreground">{formatSats(snapshot.sparkBalanceSats)}</span>
                            {snapshot.sparkStatus ? ` (${snapshot.sparkStatus})` : ''}
                        </div>
                    )}
                    {!error && snapshot?.hasWallet && snapshot.sparkLastError ? (
                        <div className="mt-1 text-[11px] text-destructive/90">{snapshot.sparkLastError}</div>
                    ) : null}
                </CardContent>
            </Card>
        );
    }

    return (
        <Card className="mx-auto mb-3 w-full max-w-[768px] gap-3 py-3">
            <CardHeader className="gap-1 px-4">
                <CardTitle className="text-sm">Lightning wallet snapshot</CardTitle>
                <CardDescription className="text-xs">
                    Live Spark balance and recent L402 receipt summary.
                </CardDescription>
                <CardAction>
                    <Button
                        type="button"
                        variant="outline"
                        size="sm"
                        disabled={loading || disabled}
                        onClick={() => {
                            capture('l402.wallet_snapshot_refresh_clicked', {
                                variant,
                                disabled,
                            });
                            void refresh();
                        }}
                    >
                        {loading ? 'Refreshing...' : 'Refresh'}
                    </Button>
                </CardAction>
            </CardHeader>
            <CardContent className="px-4">
                {error ? (
                    <div className="text-xs text-destructive">{error}</div>
                ) : (
                    <div className="grid gap-1 text-xs text-muted-foreground sm:grid-cols-2">
                        <div>
                            <span className="font-medium text-foreground">Balance:</span>{' '}
                            {snapshot?.hasWallet ? formatSats(snapshot.sparkBalanceSats) : 'No wallet yet'}
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
                        <div className="sm:col-span-2">
                            <span className="font-medium text-foreground">Last proof:</span>{' '}
                            {snapshot?.lastProofReference ?? 'none'}
                        </div>
                        <div className="sm:col-span-2">
                            <span className="font-medium text-foreground">Updated:</span>{' '}
                            {snapshot?.lastUpdatedAt ?? (snapshot?.hasWallet ? 'n/a' : 'No wallet yet')}
                        </div>
                        {snapshot?.sparkLastError ? (
                            <div className="sm:col-span-2 text-destructive/90">
                                <span className="font-medium text-foreground">Wallet error:</span>{' '}
                                {snapshot.sparkLastError}
                            </div>
                        ) : null}
                    </div>
                )}
            </CardContent>
        </Card>
    );
}
