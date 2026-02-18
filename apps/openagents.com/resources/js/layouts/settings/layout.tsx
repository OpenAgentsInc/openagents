import { Link, usePage } from '@inertiajs/react';
import type { PropsWithChildren } from 'react';
import Heading from '@/components/heading';
import { cn } from '@/lib/utils';

const SETTINGS_NAV = [
    { label: 'Profile', href: '/settings/profile' },
    { label: 'Autopilot', href: '/settings/autopilot' },
];

export default function SettingsLayout({ children }: PropsWithChildren) {
    const { url } = usePage();
    const pathname = url.split('?')[0];

    return (
        <div className="px-4 py-6">
            <Heading
                title="Settings"
                description="Manage your profile and account settings"
            />

            <div className="mb-6 flex flex-wrap gap-2">
                {SETTINGS_NAV.map((item) => {
                    const isActive = pathname === item.href;

                    return (
                        <Link
                            key={item.href}
                            href={item.href}
                            className={cn(
                                'rounded-md border px-3 py-1.5 text-sm transition-colors',
                                isActive
                                    ? 'border-foreground/40 bg-foreground/10 text-foreground'
                                    : 'border-border text-muted-foreground hover:bg-foreground/5 hover:text-foreground',
                            )}
                        >
                            {item.label}
                        </Link>
                    );
                })}
            </div>

            <div className="max-w-2xl space-y-12">{children}</div>
        </div>
    );
}
