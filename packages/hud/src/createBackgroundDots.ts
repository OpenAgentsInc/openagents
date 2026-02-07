import type { DotsOrigin } from './getDistanceFromOriginToCornerProgress.js';
import { getDistanceFromOriginToCornerProgress } from './getDistanceFromOriginToCornerProgress.js';

export interface CreateBackgroundDotsSettings {
  color?: string;
  type?: 'box' | 'circle' | 'cross';
  distance?: number;
  size?: number;
  crossSize?: number;
  origin?: DotsOrigin;
  /** If true, fade from edge to center; else center to edge */
  originInverted?: boolean;
  /** If true, all dots same opacity (no radial fade) */
  uniform?: boolean;
}

export interface BackgroundDots {
  cancel: () => void;
}

const DEFAULTS: Required<CreateBackgroundDotsSettings> = {
  color: '#777',
  type: 'box',
  distance: 30,
  size: 4,
  crossSize: 1,
  origin: 'center',
  originInverted: false,
  uniform: false,
};

/**
 * Draw a grid of dots on a canvas. No animator – draws once and on resize.
 * Adapted from @arwes/bgs createBackgroundDots (no @arwes/animated or @arwes/animator).
 */
export function createBackgroundDots(
  canvas: HTMLCanvasElement,
  settings: CreateBackgroundDotsSettings = {},
): BackgroundDots {
  const ctx = canvas.getContext('2d');
  if (!ctx) return { cancel: () => {} };
  const context = ctx;

  const dpr = Math.min(typeof window !== 'undefined' ? window.devicePixelRatio : 1, 2);
  let resizeObserver: ResizeObserver | undefined;

  const getSettings = (): Required<CreateBackgroundDotsSettings> => ({
    ...DEFAULTS,
    ...settings,
  });

  function resize(): void {
    const rect = canvas.getBoundingClientRect();
    const w = rect.width;
    const h = rect.height;
    if (canvas.width !== w * dpr || canvas.height !== h * dpr) {
      canvas.width = w * dpr;
      canvas.height = h * dpr;
    }
    context.setTransform(1, 0, 0, 1, 0, 0);
    context.scale(dpr, dpr);
  }

  function draw(): void {
    const { color, type, distance, size, crossSize, origin, originInverted, uniform } =
      getSettings();
    const width = canvas.width / dpr;
    const height = canvas.height / dpr;

    const xLength = 1 + Math.floor(width / distance);
    const yLength = 1 + Math.floor(height / distance);
    const xMargin = width % distance;
    const yMargin = height % distance;

    context.clearRect(0, 0, width, height);

    for (let xi = 0; xi < xLength; xi++) {
      const x = xMargin / 2 + xi * distance;
      for (let yi = 0; yi < yLength; yi++) {
        const y = yMargin / 2 + yi * distance;

        const progress = getDistanceFromOriginToCornerProgress(
          width,
          height,
          x,
          y,
          origin,
        );
        const alpha = uniform ? 1 : (originInverted ? progress : 1 - progress);
        context.globalAlpha = Math.max(0, Math.min(1, alpha));

        context.beginPath();
        if (type === 'circle') {
          context.arc(x, y, size, 0, 2 * Math.PI);
        } else if (type === 'cross') {
          const l = size / 2;
          const b = crossSize / 2;
          context.moveTo(x - l, y + b);
          context.lineTo(x - l, y - b);
          context.lineTo(x - b, y - b);
          context.lineTo(x - b, y - l);
          context.lineTo(x + b, y - l);
          context.lineTo(x + b, y - b);
          context.lineTo(x + l, y - b);
          context.lineTo(x + l, y + b);
          context.lineTo(x + b, y + b);
          context.lineTo(x + b, y + l);
          context.lineTo(x - b, y + l);
          context.lineTo(x - b, y + b);
        } else {
          context.rect(x - size / 2, y - size / 2, size, size);
        }
        context.fillStyle = color;
        context.fill();
        context.closePath();
      }
    }
    context.globalAlpha = 1;
  }

  function run(): void {
    if (typeof window === 'undefined') return;
    resize();
    draw();
    canvas.style.opacity = '1';
    resizeObserver = new ResizeObserver(() => {
      resize();
      draw();
    });
    resizeObserver.observe(canvas);
  }

  run();

  return {
    cancel() {
      resizeObserver?.disconnect();
      canvas.style.opacity = '0';
    },
  };
}
