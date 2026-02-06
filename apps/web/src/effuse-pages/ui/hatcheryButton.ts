import { html, rawHtml } from '@openagentsinc/effuse';

export type HatcheryButtonVariant = 'fill' | 'outline';
export type HatcheryButtonSize = 'default' | 'small' | 'large';

const FRAME_SVG = rawHtml(`
<svg
  class="oa-hatchery-button__frame"
  viewBox="0 0 100 40"
  preserveAspectRatio="none"
  role="presentation"
  aria-hidden
>
  <polygon class="oa-hatchery-button__bg" points="6,0 94,0 100,6 100,34 94,40 6,40 0,34 0,6" />
  <polygon class="oa-hatchery-button__line" points="6,0 94,0 100,6 100,34 94,40 6,40 0,34 0,6" />
</svg>
`);

export function hatcheryButton(input: {
  readonly href: string;
  readonly label: string;
  readonly variant?: HatcheryButtonVariant;
  readonly size?: HatcheryButtonSize;
  readonly className?: string;
}) {
  const variant = input.variant ?? 'fill';
  const size = input.size ?? 'default';

  const className = [
    'oa-hatchery-button',
    variant === 'outline' ? 'oa-hatchery-button--outline' : null,
    size === 'small' ? 'oa-hatchery-button--small' : null,
    size === 'large' ? 'oa-hatchery-button--large' : null,
    input.className ?? null,
  ]
    .filter(Boolean)
    .join(' ');

  return html`
    <a href="${input.href}" class="${className}">
      ${FRAME_SVG}
      <span class="oa-hatchery-button__content">${input.label}</span>
    </a>
  `;
}

