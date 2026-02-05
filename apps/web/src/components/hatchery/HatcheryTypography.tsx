import type { HTMLAttributes, ReactNode } from 'react';
import clsx from 'clsx';

type HatcheryTextProps<E extends HTMLElement> = HTMLAttributes<E> & {
  children: ReactNode;
  durationMs?: number;
};

export function HatcheryH1(props: HatcheryTextProps<HTMLHeadingElement>) {
  const { className, children, durationMs: _durationMs, ...rest } = props;
  return (
    <h1
      className={clsx('font-semibold text-2xl text-foreground', className)}
      {...rest}
    >
      {children}
    </h1>
  );
}

export function HatcheryH2(props: HatcheryTextProps<HTMLHeadingElement>) {
  const { className, children, durationMs: _durationMs, ...rest } = props;
  return (
    <h2
      className={clsx('font-medium text-lg text-foreground', className)}
      {...rest}
    >
      {children}
    </h2>
  );
}

export function HatcheryP(props: HatcheryTextProps<HTMLParagraphElement>) {
  const { className, children, durationMs: _durationMs, ...rest } = props;
  return (
    <p
      className={clsx('text-muted-foreground text-sm', className)}
      {...rest}
    >
      {children}
    </p>
  );
}
