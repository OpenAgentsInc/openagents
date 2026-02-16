import { Head, Link } from '@inertiajs/react';
import { chat } from '@/routes';

export default function DashboardFallback() {
    return (
        <div className="flex min-h-screen items-center justify-center bg-background p-6 text-foreground">
            <Head title="Chat" />
            <div className="w-full max-w-md rounded-lg border border-border bg-card p-6 text-center">
                <h1 className="text-lg font-semibold">Dashboard was replaced</h1>
                <p className="mt-2 text-sm text-muted-foreground">
                    OpenAgents now starts in chat.
                </p>
                <Link
                    href={chat()}
                    className="mt-4 inline-flex rounded-md bg-primary px-4 py-2 text-sm font-medium text-primary-foreground"
                >
                    Open chat
                </Link>
            </div>
        </div>
    );
}
