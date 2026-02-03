import clsx from 'clsx';
import type { ComponentProps } from 'react';

export function Logo({ className, ...props }: ComponentProps<'span'>) {
  return <span className={clsx('flex h-8 items-stretch', className)} {...props} />;
}

export function LogoGrid({ className, ...props }: ComponentProps<'div'>) {
  return (
    <div
      className={clsx(
        'mx-auto grid w-full grid-cols-2 place-items-center gap-x-6 gap-y-10 sm:grid-cols-3 sm:gap-x-10 lg:mx-auto lg:inline-grid lg:auto-cols-fr lg:grid-flow-col lg:grid-cols-1 lg:gap-12',
        className,
      )}
      {...props}
    />
  );
}
