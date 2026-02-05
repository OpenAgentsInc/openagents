export interface CreateBackgroundGridLinesSettings {
  lineWidth?: number;
  lineColor?: string;
  horizontalLineDash?: number[];
  verticalLineDash?: number[];
  distance?: number;
}

export interface BackgroundGridLines {
  cancel: () => void;
}

const DEFAULTS: Required<CreateBackgroundGridLinesSettings> = {
  lineWidth: 1,
  lineColor: '#777',
  horizontalLineDash: [4],
  verticalLineDash: [],
  distance: 30,
};

/**
 * Draw a grid of lines on a canvas. No animator – draws once and on resize.
 * Adapted from @arwes/bgs createBackgroundGridLines.
 */
export function createBackgroundGridLines(
  canvas: HTMLCanvasElement,
  settings: CreateBackgroundGridLinesSettings = {},
): BackgroundGridLines {
  const ctx = canvas.getContext('2d');
  if (!ctx) return { cancel: () => {} };
  const context = ctx;

  const dpr = Math.min(typeof window !== 'undefined' ? window.devicePixelRatio : 1, 2);
  let resizeObserver: ResizeObserver | undefined;

  const getSettings = (): Required<CreateBackgroundGridLinesSettings> => ({
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
    const { lineWidth, lineColor, horizontalLineDash, verticalLineDash, distance } =
      getSettings();
    const width = canvas.width / dpr;
    const height = canvas.height / dpr;

    const xLength = 1 + Math.floor(width / distance);
    const yLength = 1 + Math.floor(height / distance);
    const xMargin = width % distance;
    const yMargin = height % distance;

    context.clearRect(0, 0, width, height);
    context.lineWidth = lineWidth;
    context.strokeStyle = lineColor;

    context.setLineDash(horizontalLineDash);
    for (let yi = 0; yi < yLength; yi++) {
      const y = yMargin / 2 + yi * distance;
      context.beginPath();
      context.moveTo(0, y);
      context.lineTo(width, y);
      context.stroke();
      context.closePath();
    }

    context.setLineDash(verticalLineDash);
    for (let xi = 0; xi < xLength; xi++) {
      const x = xMargin / 2 + xi * distance;
      context.beginPath();
      context.moveTo(x, 0);
      context.lineTo(x, height);
      context.stroke();
      context.closePath();
    }
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
