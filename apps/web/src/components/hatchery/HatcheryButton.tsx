import styles from './HatcheryButton.module.css';
import type { AnchorHTMLAttributes, ButtonHTMLAttributes, ReactNode } from 'react';

type HatcheryButtonVariant = 'fill' | 'outline';

type BaseProps = {
  variant?: HatcheryButtonVariant;
  size?: 'default' | 'small';
  children: ReactNode;
  className?: string;
};

type ButtonProps = BaseProps & ButtonHTMLAttributes<HTMLButtonElement> & { href?: never };
type LinkProps = BaseProps & AnchorHTMLAttributes<HTMLAnchorElement> & { href: string };

export function HatcheryButton({
  variant = 'fill',
  size = 'default',
  className,
  children,
  ...props
}: ButtonProps | LinkProps) {
  const classNames = [
    styles.root,
    variant === 'outline' ? styles.outline : undefined,
    size === 'small' ? styles.small : undefined,
    className,
  ]
    .filter(Boolean)
    .join(' ');

  const content = (
    <>
      <svg
        className={styles.frame}
        viewBox="0 0 100 40"
        preserveAspectRatio="none"
        role="presentation"
      >
        <polygon
          className={styles.bg}
          points="6,0 94,0 100,6 100,34 94,40 6,40 0,34 0,6"
        />
        <polygon
          className={styles.line}
          points="6,0 94,0 100,6 100,34 94,40 6,40 0,34 0,6"
        />
      </svg>
      <span className={styles.content}>{children}</span>
    </>
  );

  if ('href' in props && props.href) {
    const { href, ...rest } = props as LinkProps;
    return (
      <a {...rest} href={href} className={classNames}>
        {content}
      </a>
    );
  }

  return (
    <button {...(props as ButtonHTMLAttributes<HTMLButtonElement>)} className={classNames}>
      {content}
    </button>
  );
}
