import type { ButtonHTMLAttributes, ReactNode } from 'react';
import styles from './HatcheryButton.module.css';

type HatcheryButtonVariant = 'fill' | 'outline';

interface HatcheryButtonProps extends ButtonHTMLAttributes<HTMLButtonElement> {
  variant?: HatcheryButtonVariant;
  size?: 'default' | 'small';
  children: ReactNode;
}

export function HatcheryButton({
  variant = 'fill',
  size = 'default',
  className,
  children,
  ...props
}: HatcheryButtonProps) {
  return (
    <button
      {...props}
      className={[
        styles.root,
        variant === 'outline' ? styles.outline : undefined,
        size === 'small' ? styles.small : undefined,
        className,
      ]
        .filter(Boolean)
        .join(' ')}
    >
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
    </button>
  );
}
