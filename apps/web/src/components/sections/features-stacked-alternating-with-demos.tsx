import clsx from 'clsx';
import type { ComponentProps, ReactNode } from 'react';
import { Section } from '../elements/section';

export function Feature({
  headline,
  subheadline,
  cta,
  demo,
  className,
}: {
  headline: ReactNode;
  subheadline: ReactNode;
  cta: ReactNode;
  demo: ReactNode;
} & Omit<ComponentProps<'div'>, 'children'>) {
  return (
    <div
      className={clsx(
        'group grid grid-flow-dense grid-cols-1 gap-2 rounded-lg bg-mauve-950/2.5 p-2 lg:grid-cols-2 dark:bg-white/5',
        className,
      )}
    >
      <div className="flex flex-col justify-between gap-6 p-6 sm:gap-10 sm:p-10 lg:p-6 lg:group-even:col-start-2">
        <div className="text-xl/8 sm:text-2xl/9">
          <h3 className="text-mauve-950 dark:text-white">{headline}</h3>
          <div className="flex flex-col gap-4 text-mauve-500">{subheadline}</div>
        </div>
        {cta}
      </div>
      <div className="relative overflow-hidden rounded-sm lg:group-even:col-start-1 dark:after:absolute dark:after:inset-0 dark:after:rounded-sm dark:after:outline-1 dark:after:-outline-offset-1 dark:after:outline-white/10">
        {demo}
      </div>
    </div>
  );
}

export function FeaturesStackedAlternatingWithDemos({
  features,
  ...props
}: { features: ReactNode } & Omit<ComponentProps<typeof Section>, 'children'>) {
  return (
    <Section {...props}>
      <div className="grid grid-cols-1 gap-6">{features}</div>
    </Section>
  );
}
