import clsx from 'clsx';
import type { ComponentProps, ReactNode } from 'react';
import { Container } from '../elements/container';

export function FooterLink({ href, className, ...props }: { href: string } & Omit<ComponentProps<'a'>, 'href'>) {
  return (
    <li className={clsx('text-mauve-700 dark:text-mauve-400', className)}>
      <a href={href} {...props} />
    </li>
  );
}

export function SocialLink({
  href,
  name,
  className,
  ...props
}: {
  href: string;
  name: string;
} & Omit<ComponentProps<'a'>, 'href'>) {
  return (
    <a
      href={href}
      target="_blank"
      rel="noreferrer"
      aria-label={name}
      className={clsx('text-mauve-950 *:size-6 dark:text-white', className)}
      {...props}
    />
  );
}

export function FooterWithLinksAndSocialIcons({
  links,
  socialLinks,
  fineprint,
  className,
  ...props
}: {
  links: ReactNode;
  socialLinks?: ReactNode;
  fineprint: ReactNode;
} & ComponentProps<'footer'>) {
  return (
    <footer className={clsx('pt-16', className)} {...props}>
      <div className="bg-mauve-950/2.5 py-16 text-mauve-950 dark:bg-white/5 dark:text-white">
        <Container className="flex flex-col gap-10 text-center text-sm/7">
          <div className="flex flex-col gap-6">
            <nav>
              <ul className="flex flex-wrap items-center justify-center gap-x-10 gap-y-2">{links}</ul>
            </nav>
            {socialLinks && <div className="flex items-center justify-center gap-10">{socialLinks}</div>}
          </div>
          <div className="text-mauve-600 dark:text-mauve-500">{fineprint}</div>
        </Container>
      </div>
    </footer>
  );
}
