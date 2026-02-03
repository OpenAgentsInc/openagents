import clsx from 'clsx';
import type { ComponentProps, ReactNode } from 'react';
import { useCallback, useState } from 'react';
import { CheckmarkIcon } from '../icons/checkmark-icon';
import { Squares2StackedIcon } from '../icons/squares-2-stacked-icon';

export function InstallCommand({
  snippet,
  variant = 'normal',
  className,
  ...props
}: {
  snippet: ReactNode;
  variant?: 'normal' | 'overlay';
} & ComponentProps<'div'>) {
  const [copied, setCopied] = useState(false);
  const copy = useCallback(() => {
    const text = typeof snippet === 'string' ? snippet : String(snippet);
    void navigator.clipboard.writeText(text).then(() => {
      setCopied(true);
      setTimeout(() => setCopied(false), 2000);
    });
  }, [snippet]);

  return (
    <div
      className={clsx(
        'flex items-center justify-between gap-6 rounded-full p-1 font-mono text-sm/7 inset-ring-1 dark:bg-white/10 dark:inset-ring-white/10',
        variant === 'normal' && 'bg-white text-mauve-600 inset-ring-black/10 dark:text-white',
        variant === 'overlay' && 'bg-white/15 text-white inset-ring-white/10',
        className,
      )}
      {...props}
    >
      <div className="flex items-center gap-2 pl-3">
        <div className="text-current/60 select-none">$</div>
        <span id="snippet">{snippet}</span>
      </div>
      <button
        type="button"
        onClick={copy}
        className="group relative flex size-9 items-center justify-center rounded-full after:absolute after:-inset-1 hover:bg-mauve-950/10 dark:hover:bg-white/10 after:pointer-events-none"
      >
        {copied ? <CheckmarkIcon /> : <Squares2StackedIcon />}
      </button>
    </div>
  );
}
