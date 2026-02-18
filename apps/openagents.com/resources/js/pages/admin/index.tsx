import { Head } from '@inertiajs/react';

export default function AdminIndex() {
    return (
        <>
            <Head title="Admin" />
            <div className="flex h-full flex-1 flex-col gap-4 rounded-xl p-4">
                <div className="rounded-xl border border-sidebar-border/70 bg-card p-6 dark:border-sidebar-border">
                    <h1 className="text-xl font-semibold">Admin Area</h1>
                    <p className="mt-2 text-sm text-muted-foreground">
                        This area is restricted to configured admin emails.
                    </p>
                </div>
            </div>
        </>
    );
}
