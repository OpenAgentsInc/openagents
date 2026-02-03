import clsx from 'clsx';
import type { ComponentProps } from 'react';

const sizes = {
  md: 'px-3 py-1',
  lg: 'px-4 py-2',
};

export function ButtonLink({
  size = 'md',
  color = 'dark/light',
  className,
  href,
  ...props
}: {
  href: string;
  size?: keyof typeof sizes;
  color?: 'dark/light' | 'light';
} & Omit<ComponentProps<'a'>, 'href'>) {
  return (
    <a
      href={href}
      className={clsx(
        'inline-flex shrink-0 items-center justify-center gap-1 rounded-full text-sm/7 font-medium',
        color === 'dark/light' &&
          'bg-mauve-950 text-white hover:bg-mauve-800 dark:bg-mauve-300 dark:text-mauve-950 dark:hover:bg-mauve-200',
        color === 'light' && 'hover bg-white text-mauve-950 hover:bg-mauve-100 dark:bg-mauve-100 dark:hover:bg-white',
        sizes[size],
        className,
      )}
      {...props}
    />
  );
}

export function PlainButtonLink({
  size = 'md',
  color = 'dark/light',
  href,
  className,
  ...props
}: {
  href: string;
  size?: keyof typeof sizes;
  color?: 'dark/light' | 'light';
} & Omit<ComponentProps<'a'>, 'href'>) {
  return (
    <a
      href={href}
      className={clsx(
        'inline-flex shrink-0 items-center justify-center gap-2 rounded-full text-sm/7 font-medium',
        color === 'dark/light' && 'text-mauve-950 hover:bg-mauve-950/10 dark:text-white dark:hover:bg-white/10',
        color === 'light' && 'text-white hover:bg-white/15 dark:hover:bg-white/10',
        sizes[size],
        className,
      )}
      {...props}
    />
  );
}
