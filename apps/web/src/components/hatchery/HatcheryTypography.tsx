import type { HTMLAttributes, ReactNode } from 'react';
import { useEffect, useMemo, useState } from 'react';
import clsx from 'clsx';

type HatcheryTextProps<E extends HTMLElement> = HTMLAttributes<E> & {
  children: ReactNode;
  durationMs?: number;
};

const CHARSET = 'ABCDEFGHIJKLMNOPQRSTUVWXYZ0123456789';

const useDecipherText = (text: string, durationMs: number) => {
  const [display, setDisplay] = useState(text);

  useEffect(() => {
    if (!text) {
      setDisplay(text);
      return;
    }

    let rafId = 0;
    const start = performance.now();
    const length = text.length;

    const tick = (now: number) => {
      const progress = Math.min(1, (now - start) / durationMs);
      const revealCount = Math.floor(progress * length);
      const next = text
        .split('')
        .map((char, idx) => {
          if (char === ' ') {
            return ' ';
          }
          if (idx < revealCount) {
            return char;
          }
          return CHARSET[Math.floor(Math.random() * CHARSET.length)];
        })
        .join('');

      setDisplay(next);

      if (progress < 1) {
        rafId = requestAnimationFrame(tick);
      } else {
        setDisplay(text);
      }
    };

    rafId = requestAnimationFrame(tick);

    return () => {
      if (rafId) {
        cancelAnimationFrame(rafId);
      }
    };
  }, [text, durationMs]);

  return display;
};

const AnimatedText = ({
  as: Element,
  className,
  children,
  durationMs = 600,
  ...rest
}: {
  as: keyof HTMLElementTagNameMap;
  className?: string;
  children: ReactNode;
  durationMs?: number;
} & HTMLAttributes<HTMLElement>) => {
  const text = useMemo(() => {
    if (typeof children === 'string' || typeof children === 'number') {
      return String(children);
    }
    return '';
  }, [children]);

  const display = useDecipherText(text, durationMs);

  return (
    <Element className={className} {...rest}>
      {text ? display : children}
    </Element>
  );
};

export function HatcheryH1(props: HatcheryTextProps<HTMLHeadingElement>) {
  const { className, children, durationMs = 700, ...rest } = props;
  return (
    <AnimatedText
      as="h1"
      durationMs={durationMs}
      className={clsx('font-semibold text-2xl text-foreground', className)}
      {...rest}
    >
      {children}
    </AnimatedText>
  );
}

export function HatcheryH2(props: HatcheryTextProps<HTMLHeadingElement>) {
  const { className, children, durationMs = 650, ...rest } = props;
  return (
    <AnimatedText
      as="h2"
      durationMs={durationMs}
      className={clsx('font-medium text-lg text-foreground', className)}
      {...rest}
    >
      {children}
    </AnimatedText>
  );
}

export function HatcheryP(props: HatcheryTextProps<HTMLParagraphElement>) {
  const { className, children, durationMs = 500, ...rest } = props;
  return (
    <AnimatedText
      as="p"
      durationMs={durationMs}
      className={clsx('text-muted-foreground text-sm', className)}
      {...rest}
    >
      {children}
    </AnimatedText>
  );
}
