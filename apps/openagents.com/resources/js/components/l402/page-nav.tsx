import { Link } from '@inertiajs/react';
import { CircleDollarSign, Landmark, List, Server, Wallet } from 'lucide-react';
import { useCurrentUrl } from '@/hooks/use-current-url';

const links = [
    { href: '/l402', label: 'Wallet', icon: Wallet },
    { href: '/l402/transactions', label: 'Transactions', icon: List },
    { href: '/l402/paywalls', label: 'Paywalls', icon: Landmark },
    { href: '/l402/settlements', label: 'Settlements', icon: CircleDollarSign },
    { href: '/l402/deployments', label: 'Deployments', icon: Server },
] as const;

export function L402PageNav() {
    const { isCurrentUrl } = useCurrentUrl();

    return (
        <div className="mb-4 flex flex-wrap gap-2">
            {links.map((item) => {
                const Icon = item.icon;
                const active = isCurrentUrl(item.href);

                return (
                    <Link
                        key={item.href}
                        href={item.href}
                        prefetch
                        className={`inline-flex items-center gap-1 rounded-md border px-3 py-1.5 text-xs transition-colors ${
                            active
                                ? 'border-primary bg-primary/10 text-primary'
                                : 'border-sidebar-border/70 text-muted-foreground hover:bg-muted hover:text-foreground'
                        }`}
                    >
                        <Icon className="size-3.5" />
                        <span>{item.label}</span>
                    </Link>
                );
            })}
        </div>
    );
}
