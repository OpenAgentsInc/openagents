import { Head } from '@inertiajs/react';
import { KhalaSyncClient, MemoryWatermarkStore, type SyncUpdateBatch } from '@openagentsinc/khala-sync';
import { useCallback, useEffect, useMemo, useRef, useState } from 'react';
import { Badge } from '@/components/ui/badge';
import { Button } from '@/components/ui/button';
import { Input } from '@/components/ui/input';
import { Textarea } from '@/components/ui/textarea';

type WorkerStatus = 'starting' | 'running' | 'stopping' | 'stopped' | 'failed';

type ConvexProjection = {
    document_id: string;
    last_runtime_seq: number;
    lag_events: number;
    status: 'in_sync' | 'lagging';
    projection_version: string;
    last_projected_at: string | null;
};

type CodexWorkerSummary = {
    worker_id: string;
    status: WorkerStatus;
    latest_seq: number;
    workspace_ref: string | null;
    codex_home_ref: string | null;
    adapter: string;
    metadata: Record<string, unknown>;
    started_at: string | null;
    stopped_at: string | null;
    last_heartbeat_at: string | null;
    updated_at: string | null;
    convex_projection: ConvexProjection | null;
};

type CodexWorkerSnapshot = {
    worker_id: string;
    status: WorkerStatus;
    latest_seq: number;
    workspace_ref: string | null;
    codex_home_ref: string | null;
    adapter: string;
    metadata: Record<string, unknown>;
    started_at: string | null;
    stopped_at: string | null;
    updated_at: string | null;
};

type StreamEventPayload = {
    workerId: string;
    seq: number;
    eventType: string;
    payload: Record<string, unknown>;
    occurredAt: string;
};

type StreamEventRecord = {
    seq: number;
    eventType: string;
    payload: Record<string, unknown>;
    occurredAt: string;
};

type WorkerListResponse = {
    data: CodexWorkerSummary[];
};

type WorkerSnapshotResponse = {
    data: CodexWorkerSnapshot;
};

type WorkerCreateResponse = {
    data: {
        workerId: string;
        status: string;
        latestSeq: number;
        idempotentReplay: boolean;
    };
};

type WorkerRequestResponse = {
    data: {
        worker_id: string;
        request_id: string;
        ok: boolean;
        response: unknown;
    };
};

type WorkerStopResponse = {
    data: {
        worker_id: string;
        status: string;
        idempotent_replay: boolean;
    };
};

type StreamState = 'idle' | 'connecting' | 'open' | 'error';
type SyncTokenResponse = { data: { token: string } };

const khalaSyncEnabled = import.meta.env.VITE_KHALA_SYNC_ENABLED === 'true';
const khalaSyncWsUrl =
    (import.meta.env.VITE_KHALA_SYNC_WS_URL as string | undefined)?.trim() ||
    '';

function safeJsonParse(value: string): unknown {
    try {
        return JSON.parse(value) as unknown;
    } catch {
        return null;
    }
}

function isObjectRecord(value: unknown): value is Record<string, unknown> {
    return typeof value === 'object' && value !== null && !Array.isArray(value);
}

async function apiRequest<T>(url: string, init?: RequestInit): Promise<T> {
    const headers = new Headers(init?.headers ?? {});
    headers.set('Accept', 'application/json');

    if (init?.body && !headers.has('Content-Type')) {
        headers.set('Content-Type', 'application/json');
    }

    const response = await fetch(url, {
        ...init,
        headers,
    });

    const text = await response.text();
    const parsed = text === '' ? null : safeJsonParse(text);

    if (!response.ok) {
        const message =
            isObjectRecord(parsed) &&
            isObjectRecord(parsed.error) &&
            typeof parsed.error.message === 'string'
                ? parsed.error.message
                : `Request failed (${response.status})`;

        throw new Error(message);
    }

    return parsed as T;
}

function statusClass(status: WorkerStatus): string {
    if (status === 'running')
        return 'border-emerald-500/30 bg-emerald-500/10 text-emerald-300';
    if (status === 'starting')
        return 'border-sky-500/30 bg-sky-500/10 text-sky-300';
    if (status === 'stopping')
        return 'border-amber-500/30 bg-amber-500/10 text-amber-300';
    if (status === 'stopped')
        return 'border-zinc-500/30 bg-zinc-500/10 text-zinc-300';
    return 'border-rose-500/30 bg-rose-500/10 text-rose-300';
}

function projectionClass(status: ConvexProjection['status']): string {
    if (status === 'in_sync')
        return 'border-emerald-500/30 bg-emerald-500/10 text-emerald-300';

    return 'border-amber-500/30 bg-amber-500/10 text-amber-300';
}

function formatTimestamp(value: string | null): string {
    if (!value) return 'n/a';

    const date = new Date(value);
    if (Number.isNaN(date.getTime())) return value;

    return date.toLocaleString();
}

function parseJsonObject(
    value: string,
    fieldLabel: string,
): Record<string, unknown> {
    const trimmed = value.trim();
    if (trimmed === '') return {};

    const parsed = safeJsonParse(trimmed);
    if (!isObjectRecord(parsed)) {
        throw new Error(`${fieldLabel} must be a JSON object`);
    }

    return parsed;
}

function parseWorkerStatus(value: unknown): WorkerStatus | null {
    if (value === 'starting') return 'starting';
    if (value === 'running') return 'running';
    if (value === 'stopping') return 'stopping';
    if (value === 'stopped') return 'stopped';
    if (value === 'failed') return 'failed';
    return null;
}

function inferWorkerId(payload: Record<string, unknown>, docKey: string): string {
    const fromPayload =
        typeof payload.worker_id === 'string' && payload.worker_id.trim() !== ''
            ? payload.worker_id.trim()
            : '';

    if (fromPayload !== '') return fromPayload;

    const prefix = 'runtime/codex_worker_summary:';
    if (docKey.startsWith(prefix)) {
        return docKey.slice(prefix.length);
    }

    return '';
}

function workerSummaryFromSyncUpdate(
    update: SyncUpdateBatch['updates'][number],
    existing: CodexWorkerSummary,
): CodexWorkerSummary | null {
    if (!isObjectRecord(update.payload)) {
        return null;
    }

    const payload = update.payload;
    const workerId = inferWorkerId(payload, update.doc_key);
    if (workerId === '' || workerId !== existing.worker_id) {
        return null;
    }

    const nextStatus = parseWorkerStatus(payload.status) ?? existing.status;
    const nextLatestSeq =
        typeof payload.latest_seq === 'number' && Number.isFinite(payload.latest_seq)
            ? payload.latest_seq
            : existing.latest_seq;

    return {
        ...existing,
        worker_id: workerId,
        status: nextStatus,
        latest_seq: nextLatestSeq,
        workspace_ref:
            typeof payload.workspace_ref === 'string'
                ? payload.workspace_ref
                : payload.workspace_ref === null
                  ? null
                  : existing.workspace_ref,
        codex_home_ref:
            typeof payload.codex_home_ref === 'string'
                ? payload.codex_home_ref
                : payload.codex_home_ref === null
                  ? null
                  : existing.codex_home_ref,
        adapter:
            typeof payload.adapter === 'string' && payload.adapter.trim() !== ''
                ? payload.adapter
                : existing.adapter,
        metadata: isObjectRecord(payload.metadata) ? payload.metadata : existing.metadata,
        started_at:
            typeof payload.started_at === 'string'
                ? payload.started_at
                : payload.started_at === null
                  ? null
                  : existing.started_at,
        stopped_at:
            typeof payload.stopped_at === 'string'
                ? payload.stopped_at
                : payload.stopped_at === null
                  ? null
                  : existing.stopped_at,
        last_heartbeat_at:
            typeof payload.last_heartbeat_at === 'string'
                ? payload.last_heartbeat_at
                : payload.last_heartbeat_at === null
                  ? null
                  : existing.last_heartbeat_at,
        updated_at:
            typeof payload.updated_at === 'string'
                ? payload.updated_at
                : payload.updated_at === null
                  ? null
                  : existing.updated_at,
    };
}

export default function AdminIndex() {
    const [workers, setWorkers] = useState<CodexWorkerSummary[]>([]);
    const [workersLoading, setWorkersLoading] = useState(false);
    const [workersError, setWorkersError] = useState<string | null>(null);

    const [statusFilter, setStatusFilter] = useState<'all' | WorkerStatus>(
        'all',
    );
    const [workspaceFilter, setWorkspaceFilter] = useState('');

    const [selectedWorkerId, setSelectedWorkerId] = useState<string | null>(
        null,
    );
    const [selectedSnapshot, setSelectedSnapshot] =
        useState<CodexWorkerSnapshot | null>(null);
    const [snapshotLoading, setSnapshotLoading] = useState(false);
    const [snapshotError, setSnapshotError] = useState<string | null>(null);

    const [streamState, setStreamState] = useState<StreamState>('idle');
    const [streamError, setStreamError] = useState<string | null>(null);
    const [streamEvents, setStreamEvents] = useState<StreamEventRecord[]>([]);

    const [actionError, setActionError] = useState<string | null>(null);
    const [actionMessage, setActionMessage] = useState<string | null>(null);
    const [createBusy, setCreateBusy] = useState(false);
    const [requestBusy, setRequestBusy] = useState(false);
    const [stopBusy, setStopBusy] = useState(false);

    const [createWorkerId, setCreateWorkerId] = useState('');
    const [createWorkspaceRef, setCreateWorkspaceRef] = useState('');
    const [createCodexHomeRef, setCreateCodexHomeRef] = useState('');
    const [createAdapter, setCreateAdapter] = useState('in_memory');
    const [createMetadata, setCreateMetadata] = useState('{}');

    const [requestId, setRequestId] = useState('');
    const [requestMethod, setRequestMethod] = useState('thread/start');
    const [requestParams, setRequestParams] = useState(
        '{"prompt":"hello from admin"}',
    );

    const [stopReason, setStopReason] = useState('admin_stop');

    const refreshAtRef = useRef(0);
    const streamCursorRef = useRef(0);
    const loadWorkersRef = useRef<() => Promise<void>>(async () => {});
    const khalaClientRef = useRef<KhalaSyncClient | null>(null);

    const selectedWorkerSummary = useMemo(
        () =>
            workers.find((worker) => worker.worker_id === selectedWorkerId) ??
            null,
        [workers, selectedWorkerId],
    );

    const loadWorkers = useCallback(async () => {
        setWorkersLoading(true);
        setWorkersError(null);

        try {
            const params = new URLSearchParams();
            params.set('limit', '50');

            if (statusFilter !== 'all') {
                params.set('status', statusFilter);
            }

            const workspace = workspaceFilter.trim();
            if (workspace !== '') {
                params.set('workspace_ref', workspace);
            }

            const query = params.toString();
            const path = query
                ? `/api/runtime/codex/workers?${query}`
                : '/api/runtime/codex/workers';
            const response = await apiRequest<WorkerListResponse>(path);
            const nextWorkers = Array.isArray(response.data)
                ? response.data
                : [];

            setWorkers(nextWorkers);
            setSelectedWorkerId((current) => {
                if (
                    current &&
                    nextWorkers.some((worker) => worker.worker_id === current)
                ) {
                    return current;
                }

                return nextWorkers.length > 0 ? nextWorkers[0].worker_id : null;
            });
        } catch (error) {
            const message =
                error instanceof Error
                    ? error.message
                    : 'Failed to load workers';
            setWorkersError(message);
        } finally {
            setWorkersLoading(false);
        }
    }, [statusFilter, workspaceFilter]);

    const loadSnapshot = useCallback(
        async (workerId: string, silent = false) => {
            if (!silent) {
                setSnapshotLoading(true);
            }

            setSnapshotError(null);

            try {
                const response = await apiRequest<WorkerSnapshotResponse>(
                    `/api/runtime/codex/workers/${encodeURIComponent(workerId)}`,
                );
                setSelectedSnapshot(response.data);
            } catch (error) {
                const message =
                    error instanceof Error
                        ? error.message
                        : 'Failed to load worker snapshot';
                setSnapshotError(message);
            } finally {
                if (!silent) {
                    setSnapshotLoading(false);
                }
            }
        },
        [],
    );

    const refreshSelectedWorker = useCallback(
        async (workerId: string, silent = false) => {
            await Promise.all([loadWorkers(), loadSnapshot(workerId, silent)]);
        },
        [loadWorkers, loadSnapshot],
    );

    useEffect(() => {
        loadWorkersRef.current = loadWorkers;
    }, [loadWorkers]);

    useEffect(() => {
        void loadWorkers();
    }, [loadWorkers]);

    useEffect(() => {
        const timer = window.setInterval(() => {
            void loadWorkers();
        }, 5000);

        return () => {
            window.clearInterval(timer);
        };
    }, [loadWorkers]);

    useEffect(() => {
        if (!khalaSyncEnabled) {
            return;
        }

        const wsUrl =
            khalaSyncWsUrl !== ''
                ? khalaSyncWsUrl
                : `${window.location.protocol === 'https:' ? 'wss:' : 'ws:'}//${window.location.host}/sync/socket/websocket`;

        let cancelled = false;

        const client = new KhalaSyncClient({
            url: wsUrl,
            watermarkStore: new MemoryWatermarkStore(),
            tokenProvider: async () => {
                const response = await apiRequest<SyncTokenResponse>(
                    '/api/sync/token',
                    {
                        method: 'POST',
                        body: JSON.stringify({
                            scopes: ['runtime.codex_worker_summaries'],
                        }),
                    },
                );

                const token = String(response.data?.token ?? '').trim();
                if (token === '') {
                    throw new Error('Sync token response missing token');
                }

                return token;
            },
            onUpdateBatch: (batch) => {
                setWorkers((previous) => {
                    if (previous.length === 0) return previous;

                    let changed = false;

                    const next = previous.map((worker) => {
                        const update = batch.updates.find((candidate) => {
                            if (
                                candidate.topic !==
                                'runtime.codex_worker_summaries'
                            ) {
                                return false;
                            }

                            if (
                                !isObjectRecord(candidate.payload) &&
                                !candidate.doc_key.includes(worker.worker_id)
                            ) {
                                return false;
                            }

                            return true;
                        });

                        if (!update) return worker;

                        const merged = workerSummaryFromSyncUpdate(update, worker);
                        if (!merged) return worker;

                        changed = true;
                        return merged;
                    });

                    return changed ? next : previous;
                });
            },
            onStaleCursor: () => {
                void loadWorkersRef.current();
            },
            onError: (error) => {
                if (!cancelled) {
                    setWorkersError(error.message);
                }
            },
        });

        khalaClientRef.current = client;

        void (async () => {
            try {
                await client.connect();
                await client.subscribe(['runtime.codex_worker_summaries']);
            } catch (error) {
                if (!cancelled) {
                    const message =
                        error instanceof Error
                            ? error.message
                            : 'Failed to connect Khala sync client';
                    setWorkersError(message);
                }
            }
        })();

        return () => {
            cancelled = true;
            khalaClientRef.current = null;
            void client.disconnect();
        };
    }, []);

    useEffect(() => {
        if (!selectedWorkerId) {
            setSelectedSnapshot(null);
            setStreamEvents([]);
            setStreamState('idle');
            setStreamError(null);
            streamCursorRef.current = 0;
            return;
        }

        setStreamEvents([]);
        setStreamError(null);
        streamCursorRef.current = Math.max(
            (selectedWorkerSummary?.latest_seq ?? 0) - 1,
            0,
        );
        void loadSnapshot(selectedWorkerId);
    }, [selectedWorkerId, selectedWorkerSummary?.latest_seq, loadSnapshot]);

    useEffect(() => {
        if (!selectedWorkerId) {
            return;
        }

        setStreamState('connecting');
        const seedCursor = Math.max(streamCursorRef.current, 0);
        const url = new URL(
            `/api/runtime/codex/workers/${encodeURIComponent(selectedWorkerId)}/stream`,
            window.location.origin,
        );

        url.searchParams.set('cursor', String(seedCursor));
        url.searchParams.set('tail_ms', '60000');

        const source = new EventSource(url.toString(), {
            withCredentials: true,
        });

        source.onopen = () => {
            setStreamState('open');
            setStreamError(null);
        };

        const onCodexWorkerEvent = (event: MessageEvent<string>) => {
            const parsed = safeJsonParse(event.data);
            if (!isObjectRecord(parsed)) {
                return;
            }

            const payload: StreamEventPayload = {
                workerId: String(parsed.workerId ?? ''),
                seq: Number(parsed.seq ?? 0),
                eventType: String(parsed.eventType ?? 'unknown'),
                payload: isObjectRecord(parsed.payload) ? parsed.payload : {},
                occurredAt: String(
                    parsed.occurredAt ?? new Date().toISOString(),
                ),
            };

            if (
                payload.workerId !== selectedWorkerId ||
                !Number.isFinite(payload.seq)
            ) {
                return;
            }

            streamCursorRef.current = Math.max(
                streamCursorRef.current,
                payload.seq,
            );

            setStreamEvents((previous) => {
                if (previous.some((entry) => entry.seq === payload.seq)) {
                    return previous;
                }

                const next: StreamEventRecord = {
                    seq: payload.seq,
                    eventType: payload.eventType,
                    payload: payload.payload,
                    occurredAt: payload.occurredAt,
                };

                return [next, ...previous].slice(0, 200);
            });

            const now = Date.now();
            if (now - refreshAtRef.current > 400) {
                refreshAtRef.current = now;
                void refreshSelectedWorker(selectedWorkerId, true);
            }
        };

        source.addEventListener(
            'codex.worker.event',
            onCodexWorkerEvent as EventListener,
        );

        source.onerror = () => {
            setStreamState('error');
            setStreamError(
                'Stream interrupted. EventSource will retry automatically.',
            );
        };

        return () => {
            source.removeEventListener(
                'codex.worker.event',
                onCodexWorkerEvent as EventListener,
            );
            source.close();
        };
    }, [selectedWorkerId, refreshSelectedWorker]);

    const handleCreateWorker = async () => {
        setCreateBusy(true);
        setActionError(null);
        setActionMessage(null);

        try {
            const payload: Record<string, unknown> = {
                metadata: parseJsonObject(createMetadata, 'metadata'),
            };

            const workerId = createWorkerId.trim();
            if (workerId !== '') {
                payload.worker_id = workerId;
            }

            const workspaceRef = createWorkspaceRef.trim();
            if (workspaceRef !== '') {
                payload.workspace_ref = workspaceRef;
            }

            const codexHomeRef = createCodexHomeRef.trim();
            if (codexHomeRef !== '') {
                payload.codex_home_ref = codexHomeRef;
            }

            const adapter = createAdapter.trim();
            if (adapter !== '') {
                payload.adapter = adapter;
            }

            const response = await apiRequest<WorkerCreateResponse>(
                '/api/runtime/codex/workers',
                {
                    method: 'POST',
                    body: JSON.stringify(payload),
                },
            );

            setActionMessage(
                `Worker ${response.data.workerId} ${response.data.idempotentReplay ? 'reattached' : 'created'}.`,
            );
            setSelectedWorkerId(response.data.workerId);
            await refreshSelectedWorker(response.data.workerId);
        } catch (error) {
            const message =
                error instanceof Error
                    ? error.message
                    : 'Failed to create worker';
            setActionError(message);
        } finally {
            setCreateBusy(false);
        }
    };

    const handleSubmitRequest = async () => {
        if (!selectedWorkerId) return;

        setRequestBusy(true);
        setActionError(null);
        setActionMessage(null);

        try {
            const params = parseJsonObject(requestParams, 'request params');
            const payload: Record<string, unknown> = {
                method: requestMethod.trim(),
                params,
            };

            const trimmedRequestId = requestId.trim();
            if (trimmedRequestId !== '') {
                payload.request_id = trimmedRequestId;
            }

            const response = await apiRequest<WorkerRequestResponse>(
                `/api/runtime/codex/workers/${encodeURIComponent(selectedWorkerId)}/requests`,
                {
                    method: 'POST',
                    body: JSON.stringify({ request: payload }),
                },
            );

            setActionMessage(
                `Request ${response.data.request_id} completed (${response.data.ok ? 'ok' : 'error'}).`,
            );

            await refreshSelectedWorker(selectedWorkerId, true);
        } catch (error) {
            const message =
                error instanceof Error
                    ? error.message
                    : 'Failed to send request';
            setActionError(message);
        } finally {
            setRequestBusy(false);
        }
    };

    const handleStopWorker = async () => {
        if (!selectedWorkerId) return;

        setStopBusy(true);
        setActionError(null);
        setActionMessage(null);

        try {
            const response = await apiRequest<WorkerStopResponse>(
                `/api/runtime/codex/workers/${encodeURIComponent(selectedWorkerId)}/stop`,
                {
                    method: 'POST',
                    body: JSON.stringify({
                        reason: stopReason.trim() || 'admin_stop',
                    }),
                },
            );

            setActionMessage(
                `Worker ${response.data.worker_id} stop ${response.data.idempotent_replay ? 'replayed' : 'accepted'}.`,
            );

            await refreshSelectedWorker(selectedWorkerId);
        } catch (error) {
            const message =
                error instanceof Error
                    ? error.message
                    : 'Failed to stop worker';
            setActionError(message);
        } finally {
            setStopBusy(false);
        }
    };

    return (
        <>
            <Head title="Admin" />
            <div className="flex h-full flex-1 flex-col gap-4 overflow-x-auto rounded-xl p-4">
                <div className="rounded-xl border border-sidebar-border/70 bg-card p-6 dark:border-sidebar-border">
                    <h1 className="text-xl font-semibold">
                        Codex Worker Admin
                    </h1>
                    <p className="mt-2 text-sm text-muted-foreground">
                        Runtime APIs remain authoritative for worker
                        lifecycle/actions. Convex projection badges are
                        read-model indicators only.
                    </p>
                </div>

                <div className="grid gap-4 xl:grid-cols-3">
                    <div className="rounded-xl border border-sidebar-border/70 bg-card p-4 xl:col-span-1">
                        <div className="mb-3 text-xs tracking-wide text-muted-foreground uppercase">
                            Worker filters
                        </div>
                        <div className="grid gap-2">
                            <label
                                className="text-xs text-muted-foreground"
                                htmlFor="status-filter"
                            >
                                Status
                            </label>
                            <select
                                id="status-filter"
                                className="h-9 rounded-md border border-input bg-transparent px-3 text-sm dark:bg-input/30"
                                value={statusFilter}
                                onChange={(event) => {
                                    setStatusFilter(
                                        event.target.value as
                                            | 'all'
                                            | WorkerStatus,
                                    );
                                }}
                            >
                                <option value="all">all</option>
                                <option value="starting">starting</option>
                                <option value="running">running</option>
                                <option value="stopping">stopping</option>
                                <option value="stopped">stopped</option>
                                <option value="failed">failed</option>
                            </select>

                            <label
                                className="text-xs text-muted-foreground"
                                htmlFor="workspace-filter"
                            >
                                Workspace ref
                            </label>
                            <Input
                                id="workspace-filter"
                                value={workspaceFilter}
                                onChange={(event) => {
                                    setWorkspaceFilter(event.target.value);
                                }}
                                placeholder="workspace://demo"
                            />

                            <Button
                                type="button"
                                variant="outline"
                                onClick={() => {
                                    void loadWorkers();
                                }}
                                disabled={workersLoading}
                            >
                                {workersLoading
                                    ? 'Refreshing...'
                                    : 'Refresh workers'}
                            </Button>
                        </div>

                        {workersError ? (
                            <div className="mt-3 rounded border border-rose-500/40 bg-rose-500/10 p-2 text-xs text-rose-300">
                                {workersError}
                            </div>
                        ) : null}

                        <div className="mt-4 space-y-2">
                            {workers.length === 0 ? (
                                <div className="rounded border border-sidebar-border/60 p-3 text-sm text-muted-foreground">
                                    No workers for this principal/filter.
                                </div>
                            ) : (
                                workers.map((worker) => {
                                    const selected =
                                        worker.worker_id === selectedWorkerId;

                                    return (
                                        <button
                                            key={worker.worker_id}
                                            type="button"
                                            onClick={() => {
                                                setSelectedWorkerId(
                                                    worker.worker_id,
                                                );
                                            }}
                                            className={`w-full rounded border px-3 py-2 text-left transition-colors ${
                                                selected
                                                    ? 'border-primary/60 bg-primary/10'
                                                    : 'border-sidebar-border/60 hover:bg-muted/40'
                                            }`}
                                        >
                                            <div className="flex items-center justify-between gap-2">
                                                <div className="truncate text-sm font-medium">
                                                    {worker.worker_id}
                                                </div>
                                                <Badge
                                                    className={statusClass(
                                                        worker.status,
                                                    )}
                                                    variant="outline"
                                                >
                                                    {worker.status}
                                                </Badge>
                                            </div>
                                            <div className="mt-1 flex items-center gap-2 text-xs text-muted-foreground">
                                                <span>
                                                    seq {worker.latest_seq}
                                                </span>
                                                {worker.convex_projection ? (
                                                    <Badge
                                                        className={projectionClass(
                                                            worker
                                                                .convex_projection
                                                                .status,
                                                        )}
                                                        variant="outline"
                                                    >
                                                        convex{' '}
                                                        {
                                                            worker
                                                                .convex_projection
                                                                .status
                                                        }
                                                    </Badge>
                                                ) : (
                                                    <Badge variant="outline">
                                                        convex pending
                                                    </Badge>
                                                )}
                                            </div>
                                        </button>
                                    );
                                })
                            )}
                        </div>
                    </div>

                    <div className="space-y-4 xl:col-span-2">
                        <div className="rounded-xl border border-sidebar-border/70 bg-card p-4">
                            <div className="mb-3 text-xs tracking-wide text-muted-foreground uppercase">
                                Create or reattach worker
                            </div>
                            <div className="grid gap-2 md:grid-cols-2">
                                <div>
                                    <label
                                        className="mb-1 block text-xs text-muted-foreground"
                                        htmlFor="create-worker-id"
                                    >
                                        Worker id (optional)
                                    </label>
                                    <Input
                                        id="create-worker-id"
                                        value={createWorkerId}
                                        onChange={(event) => {
                                            setCreateWorkerId(
                                                event.target.value,
                                            );
                                        }}
                                        placeholder="codexw_12345"
                                    />
                                </div>
                                <div>
                                    <label
                                        className="mb-1 block text-xs text-muted-foreground"
                                        htmlFor="create-adapter"
                                    >
                                        Adapter
                                    </label>
                                    <Input
                                        id="create-adapter"
                                        value={createAdapter}
                                        onChange={(event) => {
                                            setCreateAdapter(
                                                event.target.value,
                                            );
                                        }}
                                        placeholder="in_memory"
                                    />
                                </div>
                                <div>
                                    <label
                                        className="mb-1 block text-xs text-muted-foreground"
                                        htmlFor="create-workspace-ref"
                                    >
                                        Workspace ref
                                    </label>
                                    <Input
                                        id="create-workspace-ref"
                                        value={createWorkspaceRef}
                                        onChange={(event) => {
                                            setCreateWorkspaceRef(
                                                event.target.value,
                                            );
                                        }}
                                        placeholder="workspace://demo"
                                    />
                                </div>
                                <div>
                                    <label
                                        className="mb-1 block text-xs text-muted-foreground"
                                        htmlFor="create-codex-home-ref"
                                    >
                                        Codex home ref
                                    </label>
                                    <Input
                                        id="create-codex-home-ref"
                                        value={createCodexHomeRef}
                                        onChange={(event) => {
                                            setCreateCodexHomeRef(
                                                event.target.value,
                                            );
                                        }}
                                        placeholder="file:///tmp/codex-home"
                                    />
                                </div>
                            </div>
                            <div className="mt-2">
                                <label
                                    className="mb-1 block text-xs text-muted-foreground"
                                    htmlFor="create-metadata"
                                >
                                    Metadata JSON
                                </label>
                                <Textarea
                                    id="create-metadata"
                                    className="min-h-20 font-mono text-xs"
                                    value={createMetadata}
                                    onChange={(event) => {
                                        setCreateMetadata(event.target.value);
                                    }}
                                />
                            </div>
                            <div className="mt-3">
                                <Button
                                    type="button"
                                    onClick={handleCreateWorker}
                                    disabled={createBusy}
                                >
                                    {createBusy
                                        ? 'Creating...'
                                        : 'Create / Reattach worker'}
                                </Button>
                            </div>
                        </div>

                        <div className="grid gap-4 lg:grid-cols-2">
                            <div className="rounded-xl border border-sidebar-border/70 bg-card p-4">
                                <div className="mb-3 flex items-center justify-between gap-2">
                                    <div className="text-xs tracking-wide text-muted-foreground uppercase">
                                        Selected worker snapshot
                                    </div>
                                    <div className="flex items-center gap-2">
                                        <Badge
                                            variant="outline"
                                            className={
                                                streamState === 'open'
                                                    ? 'border-emerald-500/30 bg-emerald-500/10 text-emerald-300'
                                                    : streamState ===
                                                        'connecting'
                                                      ? 'border-sky-500/30 bg-sky-500/10 text-sky-300'
                                                      : streamState === 'error'
                                                        ? 'border-rose-500/30 bg-rose-500/10 text-rose-300'
                                                        : ''
                                            }
                                        >
                                            stream {streamState}
                                        </Badge>
                                    </div>
                                </div>

                                {snapshotLoading ? (
                                    <div className="text-sm text-muted-foreground">
                                        Loading snapshot...
                                    </div>
                                ) : selectedSnapshot ? (
                                    <div className="space-y-2 text-sm">
                                        <div className="flex items-center justify-between gap-2">
                                            <span className="font-medium">
                                                {selectedSnapshot.worker_id}
                                            </span>
                                            <Badge
                                                className={statusClass(
                                                    selectedSnapshot.status,
                                                )}
                                                variant="outline"
                                            >
                                                {selectedSnapshot.status}
                                            </Badge>
                                        </div>
                                        <div>
                                            latest seq:{' '}
                                            <span className="font-mono">
                                                {selectedSnapshot.latest_seq}
                                            </span>
                                        </div>
                                        <div>
                                            workspace:{' '}
                                            <span className="font-mono">
                                                {selectedSnapshot.workspace_ref ??
                                                    'n/a'}
                                            </span>
                                        </div>
                                        <div>
                                            adapter:{' '}
                                            <span className="font-mono">
                                                {selectedSnapshot.adapter}
                                            </span>
                                        </div>
                                        <div>
                                            started:{' '}
                                            {formatTimestamp(
                                                selectedSnapshot.started_at,
                                            )}
                                        </div>
                                        <div>
                                            updated:{' '}
                                            {formatTimestamp(
                                                selectedSnapshot.updated_at,
                                            )}
                                        </div>
                                        <div>
                                            stopped:{' '}
                                            {formatTimestamp(
                                                selectedSnapshot.stopped_at,
                                            )}
                                        </div>

                                        {selectedWorkerSummary?.convex_projection ? (
                                            <div className="rounded border border-sidebar-border/60 p-2 text-xs">
                                                <div className="mb-1 tracking-wide text-muted-foreground uppercase">
                                                    Convex projection summary
                                                </div>
                                                <div className="flex items-center gap-2">
                                                    <Badge
                                                        variant="outline"
                                                        className={projectionClass(
                                                            selectedWorkerSummary
                                                                .convex_projection
                                                                .status,
                                                        )}
                                                    >
                                                        {
                                                            selectedWorkerSummary
                                                                .convex_projection
                                                                .status
                                                        }
                                                    </Badge>
                                                    <span>
                                                        lag{' '}
                                                        {
                                                            selectedWorkerSummary
                                                                .convex_projection
                                                                .lag_events
                                                        }
                                                    </span>
                                                </div>
                                                <div className="mt-1 text-muted-foreground">
                                                    projected at{' '}
                                                    {formatTimestamp(
                                                        selectedWorkerSummary
                                                            .convex_projection
                                                            .last_projected_at,
                                                    )}
                                                </div>
                                            </div>
                                        ) : (
                                            <div className="rounded border border-sidebar-border/60 p-2 text-xs text-muted-foreground">
                                                Convex projection pending.
                                            </div>
                                        )}

                                        <div>
                                            <label
                                                className="mb-1 block text-xs text-muted-foreground"
                                                htmlFor="stop-reason"
                                            >
                                                Stop reason
                                            </label>
                                            <Input
                                                id="stop-reason"
                                                value={stopReason}
                                                onChange={(event) => {
                                                    setStopReason(
                                                        event.target.value,
                                                    );
                                                }}
                                            />
                                        </div>
                                        <div>
                                            <Button
                                                type="button"
                                                variant="destructive"
                                                disabled={
                                                    stopBusy ||
                                                    !selectedWorkerId
                                                }
                                                onClick={handleStopWorker}
                                            >
                                                {stopBusy
                                                    ? 'Stopping...'
                                                    : 'Stop worker'}
                                            </Button>
                                        </div>
                                    </div>
                                ) : (
                                    <div className="text-sm text-muted-foreground">
                                        Select a worker to view snapshot.
                                    </div>
                                )}

                                {snapshotError ? (
                                    <div className="mt-2 rounded border border-rose-500/40 bg-rose-500/10 p-2 text-xs text-rose-300">
                                        {snapshotError}
                                    </div>
                                ) : null}
                                {streamError ? (
                                    <div className="mt-2 rounded border border-amber-500/40 bg-amber-500/10 p-2 text-xs text-amber-300">
                                        {streamError}
                                    </div>
                                ) : null}
                            </div>

                            <div className="rounded-xl border border-sidebar-border/70 bg-card p-4">
                                <div className="mb-3 text-xs tracking-wide text-muted-foreground uppercase">
                                    Submit request
                                </div>
                                <div className="space-y-2">
                                    <div>
                                        <label
                                            className="mb-1 block text-xs text-muted-foreground"
                                            htmlFor="request-id"
                                        >
                                            Request id (optional)
                                        </label>
                                        <Input
                                            id="request-id"
                                            value={requestId}
                                            onChange={(event) => {
                                                setRequestId(
                                                    event.target.value,
                                                );
                                            }}
                                            placeholder="req_123"
                                        />
                                    </div>
                                    <div>
                                        <label
                                            className="mb-1 block text-xs text-muted-foreground"
                                            htmlFor="request-method"
                                        >
                                            Method
                                        </label>
                                        <Input
                                            id="request-method"
                                            value={requestMethod}
                                            onChange={(event) => {
                                                setRequestMethod(
                                                    event.target.value,
                                                );
                                            }}
                                            placeholder="thread/start"
                                        />
                                    </div>
                                    <div>
                                        <label
                                            className="mb-1 block text-xs text-muted-foreground"
                                            htmlFor="request-params"
                                        >
                                            Params JSON
                                        </label>
                                        <Textarea
                                            id="request-params"
                                            className="min-h-24 font-mono text-xs"
                                            value={requestParams}
                                            onChange={(event) => {
                                                setRequestParams(
                                                    event.target.value,
                                                );
                                            }}
                                        />
                                    </div>
                                    <div>
                                        <Button
                                            type="button"
                                            disabled={
                                                requestBusy || !selectedWorkerId
                                            }
                                            onClick={handleSubmitRequest}
                                        >
                                            {requestBusy
                                                ? 'Submitting...'
                                                : 'Submit request'}
                                        </Button>
                                    </div>
                                </div>
                            </div>
                        </div>

                        <div className="rounded-xl border border-sidebar-border/70 bg-card p-4">
                            <div className="mb-3 flex items-center justify-between gap-2">
                                <div className="text-xs tracking-wide text-muted-foreground uppercase">
                                    Worker stream events
                                </div>
                                <Button
                                    type="button"
                                    variant="outline"
                                    size="xs"
                                    onClick={() => {
                                        setStreamEvents([]);
                                    }}
                                >
                                    Clear
                                </Button>
                            </div>
                            {streamEvents.length === 0 ? (
                                <div className="text-sm text-muted-foreground">
                                    No events received yet.
                                </div>
                            ) : (
                                <div className="max-h-[420px] space-y-2 overflow-y-auto">
                                    {streamEvents.map((entry) => (
                                        <div
                                            key={entry.seq}
                                            className="rounded border border-sidebar-border/60 p-2"
                                        >
                                            <div className="flex items-center justify-between gap-2 text-xs">
                                                <span className="font-mono">
                                                    seq {entry.seq}
                                                </span>
                                                <span className="text-muted-foreground">
                                                    {formatTimestamp(
                                                        entry.occurredAt,
                                                    )}
                                                </span>
                                            </div>
                                            <div className="mt-1 text-sm font-medium">
                                                {entry.eventType}
                                            </div>
                                            <pre className="mt-1 overflow-x-auto rounded bg-muted/40 p-2 text-xs">
                                                {JSON.stringify(
                                                    entry.payload,
                                                    null,
                                                    2,
                                                )}
                                            </pre>
                                        </div>
                                    ))}
                                </div>
                            )}
                        </div>
                    </div>
                </div>

                {actionError ? (
                    <div className="rounded border border-rose-500/40 bg-rose-500/10 p-3 text-sm text-rose-300">
                        {actionError}
                    </div>
                ) : null}

                {actionMessage ? (
                    <div className="rounded border border-emerald-500/40 bg-emerald-500/10 p-3 text-sm text-emerald-300">
                        {actionMessage}
                    </div>
                ) : null}
            </div>
        </>
    );
}
