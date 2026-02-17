import { Head, Link } from '@inertiajs/react';
import AppLayout from '@/layouts/app-layout';
import type { BreadcrumbItem } from '@/types';

type FeedShout = {
    id: number;
    zone: string;
    body: string;
    visibility: string;
    author: {
        id: number;
        name: string;
        handle: string;
        avatar: string | null;
    };
    createdAt: string | null;
    updatedAt: string | null;
};

type FeedZone = {
    zone: string;
    count24h: number;
};

type Props = {
    feed: {
        zone: string | null;
        limit: number;
        items: FeedShout[];
        zones: FeedZone[];
    };
};

const breadcrumbs: BreadcrumbItem[] = [
    { title: 'Feed', href: '/feed' },
];

function formatTimestamp(value: string | null): string {
    if (!value) {
        return 'Unknown time';
    }

    const date = new Date(value);
    if (Number.isNaN(date.getTime())) {
        return 'Unknown time';
    }

    return new Intl.DateTimeFormat(undefined, {
        dateStyle: 'medium',
        timeStyle: 'short',
    }).format(date);
}

function feedHref(zone: string | null): string {
    if (!zone || zone === 'all') {
        return '/feed';
    }

    return `/feed?zone=${encodeURIComponent(zone)}`;
}

export default function FeedPage({ feed }: Props) {
    const activeZone = feed.zone ?? 'all';

    return (
        <AppLayout breadcrumbs={breadcrumbs}>
            <Head title="Feed" />
            <div className="flex h-full flex-1 flex-col gap-4 overflow-x-auto rounded-xl p-4">
                <div className="rounded-xl border border-sidebar-border/70 bg-card p-4">
                    <h1 className="text-lg font-semibold">Global shouts</h1>
                    <p className="mt-1 text-sm text-muted-foreground">
                        Live public posts across all zones. Filter by zone or view everything.
                    </p>
                    <div className="mt-3 flex flex-wrap items-center gap-2">
                        <Link
                            href={feedHref(null)}
                            prefetch
                            className={`rounded border px-3 py-1 text-xs transition ${
                                activeZone === 'all'
                                    ? 'border-primary bg-primary/15 text-primary'
                                    : 'border-sidebar-border/70 text-muted-foreground hover:bg-muted/40'
                            }`}
                        >
                            all
                        </Link>
                        {feed.zones.map((zone) => (
                            <Link
                                key={zone.zone}
                                href={feedHref(zone.zone)}
                                prefetch
                                className={`rounded border px-3 py-1 text-xs transition ${
                                    activeZone === zone.zone
                                        ? 'border-primary bg-primary/15 text-primary'
                                        : 'border-sidebar-border/70 text-muted-foreground hover:bg-muted/40'
                                }`}
                            >
                                {zone.zone} · {zone.count24h}
                            </Link>
                        ))}
                    </div>
                </div>

                <div className="rounded-xl border border-sidebar-border/70 bg-card p-4">
                    {feed.items.length === 0 ? (
                        <div className="rounded border border-dashed border-sidebar-border/70 p-6 text-sm text-muted-foreground">
                            No shouts in this view yet.
                        </div>
                    ) : (
                        <div className="space-y-3">
                            {feed.items.map((item) => (
                                <article
                                    key={item.id}
                                    className="rounded border border-sidebar-border/70 px-3 py-3"
                                >
                                    <div className="mb-2 flex flex-wrap items-center gap-2 text-xs text-muted-foreground">
                                        <span className="rounded border border-sidebar-border/70 px-2 py-0.5 font-medium text-foreground">
                                            {item.zone || 'global'}
                                        </span>
                                        <span className="font-medium text-foreground">
                                            {item.author.name}
                                        </span>
                                        <span>@{item.author.handle}</span>
                                        <span>·</span>
                                        <span>{formatTimestamp(item.createdAt)}</span>
                                    </div>
                                    <p className="whitespace-pre-wrap break-words text-sm leading-relaxed text-foreground">
                                        {item.body}
                                    </p>
                                </article>
                            ))}
                        </div>
                    )}
                </div>
            </div>
        </AppLayout>
    );
}
