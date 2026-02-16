import * as React from 'react';
import { Link } from '@inertiajs/react';
import { cn } from '@/lib/utils';

export type HatcheryButtonVariant = 'fill' | 'outline';
export type HatcheryButtonSize = 'default' | 'small' | 'large';

const polygonPoints = '6,0 94,0 100,6 100,34 94,40 6,40 0,34 0,6';

type HatcheryButtonProps = {
    href?: string;
    label: string;
    variant?: HatcheryButtonVariant;
    size?: HatcheryButtonSize;
    className?: string;
};

export function HatcheryButton({
    href,
    label,
    variant = 'fill',
    size = 'default',
    className,
}: HatcheryButtonProps) {
    const minHeightClass = size === 'small' ? 'min-h-8' : 'min-h-11';
    const contentSizeClass =
        size === 'small'
            ? 'px-3 py-1 gap-1.5 text-xs'
            : size === 'large'
              ? 'px-8 py-4 gap-2 text-lg'
              : 'px-6 py-3 gap-2 text-[0.9375rem]';

    const fillClass = variant === 'outline' ? 'fill-transparent' : 'fill-[hsla(0,0%,100%,0.08)]';

    const baseClass = cn(
        'group relative inline-flex max-w-full items-stretch justify-stretch',
        minHeightClass,
        'm-0 border-0 bg-transparent p-0',
        'cursor-pointer select-none no-underline',
        'text-white transition-[color,opacity] duration-200 ease-out',
        'uppercase tracking-[0.08em] font-semibold',
        'hatchery-button-font',
        'focus-visible:outline-none focus-visible:ring-2 focus-visible:ring-white/50',
        className
    );

    const svgContent = (
        <>
            <svg
                className={cn(
                    'pointer-events-none absolute inset-0 h-full w-full',
                    'opacity-75 transition-[opacity,transform] duration-200 ease-out',
                    'group-hover:opacity-100 group-hover:scale-[1.02]',
                    'group-focus-visible:opacity-100 group-focus-visible:scale-[1.02]'
                )}
                viewBox="0 0 100 40"
                preserveAspectRatio="none"
                role="presentation"
                aria-hidden
            >
                <polygon className={fillClass} points={polygonPoints} />
                <polygon
                    className={cn(
                        'fill-none stroke-[hsla(0,0%,100%,0.9)] [stroke-width:2]',
                        'transition-[stroke] duration-200 ease-out',
                        'group-hover:stroke-[hsla(0,0%,100%,1)]',
                        'group-focus-visible:stroke-[hsla(0,0%,100%,1)]'
                    )}
                    points={polygonPoints}
                />
            </svg>
            <span
                className={cn(
                    'relative flex w-full min-w-0 max-w-full flex-wrap items-center justify-center',
                    contentSizeClass,
                    'leading-[1.2] whitespace-normal text-center [overflow-wrap:anywhere]'
                )}
            >
                {label}
            </span>
        </>
    );

    if (href != null && href !== '') {
        return (
            <Link href={href} className={baseClass}>
                {svgContent}
            </Link>
        );
    }
    return (
        <button type="button" className={baseClass}>
            {svgContent}
        </button>
    );
}
