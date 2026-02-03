import clsx from 'clsx';
import type { ComponentProps, ReactNode } from 'react';
import { Container } from '../elements/container';
import { Heading } from '../elements/heading';
import { Text } from '../elements/text';

export function HeroCenteredWithDemo({
  eyebrow,
  headline,
  subheadline,
  cta,
  demo,
  footer,
  className,
  ...props
}: {
  eyebrow?: ReactNode;
  headline: ReactNode;
  subheadline: ReactNode;
  cta?: ReactNode;
  demo?: ReactNode;
  footer?: ReactNode;
} & ComponentProps<'section'>) {
  return (
    <section className={clsx('py-16', className)} {...props}>
      <Container className="flex flex-col gap-16">
        <div className="flex flex-col items-center gap-32">
          <div className="flex flex-col items-center gap-6">
            {eyebrow}
            <Heading className="max-w-5xl text-center">{headline}</Heading>
            <Text size="lg" className="flex max-w-3xl flex-col gap-4 text-center">
              {subheadline}
            </Text>
            {cta}
          </div>
          {demo}
        </div>
        {footer}
      </Container>
    </section>
  );
}
