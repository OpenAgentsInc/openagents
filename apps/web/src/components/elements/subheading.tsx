import clsx from 'clsx';
import type { ComponentProps } from 'react';

export function Subheading({ children, className, ...props }: ComponentProps<'h2'>) {
  return (
    <h2
      className={clsx(
        'font-display text-[2rem]/10 tracking-tight text-pretty text-mauve-950 sm:text-5xl/14 dark:text-white',
        className,
      )}
      {...props}
    >
      {children}
    </h2>
  );
}
