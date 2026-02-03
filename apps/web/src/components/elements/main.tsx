import clsx from 'clsx';
import type { ComponentProps } from 'react';

export function Main({ children, className, ...props }: ComponentProps<'main'>) {
  return (
    <main className={clsx('isolate overflow-clip', className)} {...props}>
      {children}
    </main>
  );
}
