import { usePage } from '@inertiajs/react';
import { useEffect } from 'react';
import type { ReactNode } from 'react';
import { SidebarProvider } from '@/components/ui/sidebar';

const BODY_CLASS = 'app-layout-fixed';

type Props = {
    children: ReactNode;
    variant?: 'header' | 'sidebar';
};

export function AppShell({ children, variant = 'header' }: Props) {
    const isOpen = usePage().props.sidebarOpen;

    useEffect(() => {
        if (variant !== 'sidebar') return;
        document.documentElement.classList.add(BODY_CLASS);
        document.body.classList.add(BODY_CLASS);
        return () => {
            document.documentElement.classList.remove(BODY_CLASS);
            document.body.classList.remove(BODY_CLASS);
        };
    }, [variant]);

    if (variant === 'header') {
        return (
            <div className="flex min-h-screen w-full flex-col">{children}</div>
        );
    }

    return <SidebarProvider defaultOpen={isOpen}>{children}</SidebarProvider>;
}
