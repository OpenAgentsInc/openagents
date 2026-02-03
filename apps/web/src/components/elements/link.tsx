import clsx from 'clsx';
import type { ComponentProps } from 'react';

export function Link({
  href,
  className,
  ...props
}: { href: string } & Omit<ComponentProps<'a'>, 'href'>) {
  return (
    <a
      href={href}
      className={clsx('inline-flex items-center gap-2 text-sm/7 font-medium text-mauve-950 dark:text-white', className)}
      {...props}
    />
  );
}
