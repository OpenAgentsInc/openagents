import { Effect } from 'effect';
import { DomServiceTag, EffuseLive, html } from '@openagentsinc/effuse';
import { createBackgroundDots, createBackgroundGridLines } from '@openagentsinc/hud';
import type {
  BackgroundDots,
  BackgroundGridLines,
  CreateBackgroundDotsSettings,
  CreateBackgroundGridLinesSettings,
} from '@openagentsinc/hud';

type HudCancel = {
  readonly cancel: () => void;
};

const hudInstances = new WeakMap<Element, HudCancel>();

export function cleanupHudBackground(container: Element): void {
  const existing = hudInstances.get(container);
  if (existing) {
    try {
      existing.cancel();
    } catch (err) {
      console.error('[HUD background] cleanup failed', err);
    }
    hudInstances.delete(container);
  }
}

function fullBleedCanvas(attrs: {
  readonly role: 'dots' | 'grid';
}): ReturnType<typeof html> {
  return html`<canvas
    data-hud="${attrs.role}"
    role="presentation"
    aria-hidden="true"
    class="pointer-events-none absolute inset-0 block h-full w-full opacity-0"
  ></canvas>`;
}

export function runHudDotsBackground(
  container: Element,
  input: {
    readonly distance: number;
    readonly dotsColor: string;
    readonly dotsSettings?: Partial<CreateBackgroundDotsSettings>;
  },
): Effect.Effect<void> {
  return Effect.gen(function* () {
    cleanupHudBackground(container);

    const dom = yield* DomServiceTag;

    yield* dom.render(
      container,
      html`<div class="relative h-full w-full">${fullBleedCanvas({ role: 'dots' })}</div>`,
    );

    const dotsCanvas = container.querySelector('canvas[data-hud="dots"]');
    if (!(dotsCanvas instanceof HTMLCanvasElement)) return;

    const dots: BackgroundDots = createBackgroundDots(dotsCanvas, {
      distance: input.distance,
      color: input.dotsColor,
      ...(input.dotsSettings ?? {}),
    });

    hudInstances.set(container, {
      cancel: () => dots.cancel(),
    });
  }).pipe(
    Effect.provide(EffuseLive),
    Effect.catchAll((err) => {
      console.error('[HUD dots background]', err);
      return Effect.void;
    }),
  );
}

export function runHudDotsGridBackground(
  container: Element,
  input: {
    readonly distance: number;
    readonly dotsColor: string;
    readonly lineColor: string;
    readonly dotsSettings?: Partial<CreateBackgroundDotsSettings>;
    readonly gridSettings?: Partial<CreateBackgroundGridLinesSettings>;
  },
): Effect.Effect<void> {
  return Effect.gen(function* () {
    cleanupHudBackground(container);

    const dom = yield* DomServiceTag;

    yield* dom.render(
      container,
      html`<div class="relative h-full w-full">
        ${fullBleedCanvas({ role: 'grid' })}
        ${fullBleedCanvas({ role: 'dots' })}
      </div>`,
    );

    const gridCanvas = container.querySelector('canvas[data-hud="grid"]');
    const dotsCanvas = container.querySelector('canvas[data-hud="dots"]');
    if (!(gridCanvas instanceof HTMLCanvasElement) || !(dotsCanvas instanceof HTMLCanvasElement)) return;

    const grid: BackgroundGridLines = createBackgroundGridLines(gridCanvas, {
      distance: input.distance,
      lineColor: input.lineColor,
      ...(input.gridSettings ?? {}),
    });

    const dots: BackgroundDots = createBackgroundDots(dotsCanvas, {
      distance: input.distance,
      color: input.dotsColor,
      ...(input.dotsSettings ?? {}),
    });

    hudInstances.set(container, {
      cancel: () => {
        grid.cancel();
        dots.cancel();
      },
    });
  }).pipe(
    Effect.provide(EffuseLive),
    Effect.catchAll((err) => {
      console.error('[HUD grid background]', err);
      return Effect.void;
    }),
  );
}
