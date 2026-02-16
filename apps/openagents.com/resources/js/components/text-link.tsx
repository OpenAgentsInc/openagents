import { Link } from '@inertiajs/react';
import type { ComponentProps } from 'react';
import { cn } from '@/lib/utils';

type Props = ComponentProps<typeof Link>;

export default function TextLink({
    className = '',
    children,
    ...props
}: Props) {
    return (
        <Link
            className={cn(
                'text-foreground underline decoration-zinc-300 underline-offset-4 transition-colors duration-300 ease-out hover:decoration-current! dark:decoration-zinc-500',
                className,
            )}
            {...props}
        >
            {children}
        </Link>
    );
}
