import { Head } from '@inertiajs/react';
import { Moon } from 'lucide-react';
import Heading from '@/components/heading';
import AppLayout from '@/layouts/app-layout';
import SettingsLayout from '@/layouts/settings/layout';
import { edit as editAppearance } from '@/routes/appearance';
import type { BreadcrumbItem } from '@/types';

const breadcrumbs: BreadcrumbItem[] = [
    {
        title: 'Appearance settings',
        href: editAppearance().url,
    },
];

export default function Appearance() {
    return (
        <AppLayout breadcrumbs={breadcrumbs}>
            <Head title="Appearance settings" />

            <h1 className="sr-only">Appearance Settings</h1>

            <SettingsLayout>
                <div className="space-y-6">
                    <Heading
                        variant="small"
                        title="Appearance settings"
                        description="The app uses dark mode for everyone."
                    />
                    <div className="flex items-center gap-3 rounded-lg border border-border bg-card p-4 text-card-foreground">
                        <Moon className="h-5 w-5 shrink-0 text-muted-foreground" />
                        <p className="text-sm text-muted-foreground">
                            Dark mode is always on. There is no theme toggle.
                        </p>
                    </div>
                </div>
            </SettingsLayout>
        </AppLayout>
    );
}
