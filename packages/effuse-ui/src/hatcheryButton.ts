import { html } from '@openagentsinc/effuse';

export type HatcheryButtonVariant = 'fill' | 'outline';
export type HatcheryButtonSize = 'default' | 'small' | 'large';

function cx(...parts: Array<string | null | undefined | false>): string {
  return parts.filter(Boolean).join(' ');
}

export function hatcheryButton(input: {
  readonly href?: string;
  readonly label: string;
  readonly variant?: HatcheryButtonVariant;
  readonly size?: HatcheryButtonSize;
  readonly className?: string;
}) {
  const variant = input.variant ?? 'fill';
  const size = input.size ?? 'default';

  const minHeightClass = size === 'small' ? 'min-h-8' : 'min-h-11';

  const contentSizeClass =
    size === 'small'
      ? 'px-3 py-1 gap-1.5 text-xs'
      : size === 'large'
        ? 'px-8 py-4 gap-2 text-lg'
        : 'px-6 py-3 gap-2 text-[0.9375rem]';

  const bgClass =
    variant === 'outline' ? 'fill-transparent' : 'fill-[hsla(0,0%,100%,0.08)]';

  const baseClass = cx(
    'group relative inline-flex max-w-full items-stretch justify-stretch',
    minHeightClass,
    'm-0 border-0 bg-transparent p-0',
    'cursor-pointer select-none no-underline',
    'text-white transition-[color,opacity] duration-200 ease-out',
    'uppercase tracking-[0.08em] font-semibold',
    'use-font-square721 [font-family:var(--font-square721)]',
    'focus-visible:outline-none',
    input.className ?? null
  );

  const svgContent = html`
    <svg
      class="${cx(
    'pointer-events-none absolute inset-0 h-full w-full',
    'opacity-75 transition-[opacity,transform] duration-200 ease-out',
    'group-hover:opacity-100 group-hover:scale-[1.02]',
    'group-focus-visible:opacity-100 group-focus-visible:scale-[1.02]'
  )}"
      viewBox="0 0 100 40"
      preserveAspectRatio="none"
      role="presentation"
      aria-hidden="true"
    >
      <polygon
        class="${bgClass}"
        points="6,0 94,0 100,6 100,34 94,40 6,40 0,34 0,6"
      />
      <polygon
        class="${cx(
    'fill-none',
    'stroke-[hsla(0,0%,100%,0.9)] [stroke-width:2]',
    'transition-[stroke] duration-200 ease-out',
    'group-hover:stroke-[hsla(0,0%,100%,1)]',
    'group-focus-visible:stroke-[hsla(0,0%,100%,1)]'
  )}"
        points="6,0 94,0 100,6 100,34 94,40 6,40 0,34 0,6"
      />
    </svg>
    <span
      class="${cx(
    'relative flex w-full min-w-0 max-w-full flex-wrap items-center justify-center',
    contentSizeClass,
    'leading-[1.2] whitespace-normal text-center [overflow-wrap:anywhere]'
  )}"
    >
      ${input.label}
    </span>
  `;

  if (input.href != null && input.href !== '') {
    return html`<a href="${input.href}" class="${baseClass}">${svgContent}</a>`;
  }
  return html`<button type="button" class="${baseClass}">${svgContent}</button>`;
}
