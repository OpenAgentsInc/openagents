import clsx from 'clsx';
import type { ComponentProps, ReactNode } from 'react';
import { Container } from '../elements/container';

export function TestimonialTwoColumnWithLargePhoto({
  quote,
  img,
  name,
  byline,
  className,
  ...props
}: {
  quote: ReactNode;
  img: ReactNode;
  name: ReactNode;
  byline: ReactNode;
} & ComponentProps<'section'>) {
  return (
    <section className={clsx('py-16', className)} {...props}>
      <Container>
        <figure className="grid grid-cols-1 gap-x-2 rounded-xl bg-mauve-950/2.5 p-2 lg:grid-cols-2 dark:bg-white/5">
          <div className="flex flex-col items-start justify-between gap-10 p-6 text-mauve-950 sm:p-10 dark:text-white">
            <blockquote
              className={
                "relative flex flex-col gap-4 text-2xl/9 text-pretty *:first:before:absolute *:first:before:inline *:first:before:-translate-x-full *:first:before:content-['\"'] *:last:after:inline *:last:after:content-['\"']"
              }
            >
              {quote}
            </blockquote>
            <figcaption className="text-sm/7">
              <p className="font-semibold">{name}</p>
              <p className="text-mauve-700 dark:text-mauve-400">{byline}</p>
            </figcaption>
          </div>
          <div className="flex overflow-hidden rounded-sm outline -outline-offset-1 outline-black/5 *:object-cover dark:outline-white/5">
            {img}
          </div>
        </figure>
      </Container>
    </section>
  );
}
