import type { ButtonHTMLAttributes, ReactNode } from 'react';
import { FrameOctagon } from '@arwes/react-frames';
import styles from './HatcheryButton.module.css';

type HatcheryButtonVariant = 'fill' | 'outline';

interface HatcheryButtonProps extends ButtonHTMLAttributes<HTMLButtonElement> {
  variant?: HatcheryButtonVariant;
  children: ReactNode;
}

export function HatcheryButton({
  variant = 'fill',
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
        className,
      ]
        .filter(Boolean)
        .join(' ')}
    >
      <FrameOctagon
        className={styles.frame}
        animated={false}
        leftBottom={false}
        rightTop={false}
        padding={2}
        squareSize={16}
        strokeWidth={2}
        style={
          {
            '--arwes-frames-bg-color':
              variant === 'outline' ? 'transparent' : 'hsla(280, 45%, 6%, 0.5)',
            '--arwes-frames-line-color': 'hsla(280, 75%, 60%, 0.9)',
            '--arwes-frames-line-filter':
              'drop-shadow(0 0 6px hsla(280, 75%, 60%, 0.45))',
          } as React.CSSProperties
        }
      />
      <span className={styles.content}>{children}</span>
    </button>
  );
}
