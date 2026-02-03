import clsx from 'clsx';
import type { ComponentProps } from 'react';

export function Text({ children, className, size = 'md', ...props }: ComponentProps<'div'> & { size?: 'md' | 'lg' }) {
  return (
    <div
      className={clsx(
        size === 'md' && 'text-base/7',
        size === 'lg' && 'text-lg/8',
        'text-mauve-700 dark:text-mauve-400',
        className,
      )}
      {...props}
    >
      {children}
    </div>
  );
}
