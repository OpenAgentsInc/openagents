import clsx from 'clsx';
import type { ComponentProps, ReactNode } from 'react';
import { Section } from '../elements/section';

export function BrandCard({
  logo,
  text,
  footnote,
  className,
  ...props
}: {
  logo: ReactNode;
  text: ReactNode;
  footnote: ReactNode;
} & ComponentProps<'div'>) {
  return (
    <div
      className={clsx('flex flex-col justify-between gap-6 rounded-xl bg-mauve-950/2.5 p-6 dark:bg-white/5', className)}
      {...props}
    >
      <div className="flex flex-col items-start gap-2">
        <div className="flex h-8 shrink-0">{logo}</div>
        <p className="text-sm/7 text-mauve-700 dark:text-mauve-400">{text}</p>
      </div>
      <p className="text-xs/6 text-mauve-700 dark:text-mauve-400">{footnote}</p>
    </div>
  );
}

export function BrandsCardsMultiColumn({ children, ...props }: ComponentProps<typeof Section>) {
  return (
    <Section {...props}>
      <div className="grid grid-cols-1 gap-2 sm:grid-cols-2 sm:has-[>:last-child:nth-child(3)]:grid-cols-1 md:has-[>:last-child:nth-child(3)]:grid-cols-3 md:has-[>:nth-child(5)]:grid-cols-3 lg:has-[>:last-child:nth-child(4n)]:grid-cols-4 lg:has-[>:last-child:nth-child(4n-1)]:not(:nth-child(3n))]:grid-cols-4">
        {children}
      </div>
    </Section>
  );
}
