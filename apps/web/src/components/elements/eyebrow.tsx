import clsx from 'clsx';
import type { ComponentProps } from 'react';

export function Eyebrow({ children, className, ...props }: ComponentProps<'div'>) {
  return (
    <div className={clsx('text-sm/7 font-semibold text-mauve-700 dark:text-mauve-400', className)} {...props}>
      {children}
    </div>
  );
}
