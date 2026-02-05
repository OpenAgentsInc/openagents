type AnimateFrameAssemblerCompatOptions = {
  element: HTMLElement | SVGElement;
  duration: number;
  isEntering?: boolean;
  enterDelayMs?: number;
  exitDelayMs?: number;
};

type AnimationControl = {
  cancel: () => void;
};

const clamp = (value: number, min = 0, max = 1) =>
  Math.max(min, Math.min(max, value));

const outSine = (t: number) => Math.sin((t * Math.PI) / 2);

const animateFrameAssemblerCompat = (
  options: AnimateFrameAssemblerCompatOptions,
): AnimationControl => {
  const {
    element,
    duration,
    isEntering = true,
    enterDelayMs = 240,
    exitDelayMs = 160,
  } = options;

  const bgs = Array.from(
    element.querySelectorAll<SVGPathElement>('[data-name=bg]'),
  );
  const lines = Array.from(
    element.querySelectorAll<SVGPathElement>('[data-name=line]'),
  );
  const decos = Array.from(
    element.querySelectorAll<SVGPathElement>('[data-name=deco]'),
  );

  lines.forEach((line) => {
    const length = line.getTotalLength();
    line.style.opacity = '1';
    line.style.strokeDasharray = String(length);
    line.dataset.length = String(length);
  });

  let cancelled = false;
  const start = performance.now();
  const durationMs = Math.max(0.001, duration) * 1000;
  const delayMs = isEntering ? enterDelayMs : exitDelayMs;

  const update = () => {
    if (cancelled) {
      return;
    }

    const elapsed = performance.now() - start;
    const t = clamp(elapsed / durationMs);
    const progress = isEntering ? t : 1 - t;
    const eased = outSine(progress);

    const bgT = clamp((elapsed - delayMs) / durationMs);
    const bgProgress = isEntering ? bgT : 1 - bgT;
    const bgEased = outSine(bgProgress);

    bgs.forEach((bg) => {
      bg.style.opacity = String(bgEased);
    });

    const decoT = clamp((bgProgress - 0.5) / 0.5);
    let decoOpacity = 0;
    if (decoT > 0) {
      if (decoT < 0.33) {
        decoOpacity = decoT / 0.33;
      } else if (decoT < 0.66) {
        decoOpacity = 1 - ((decoT - 0.33) / 0.33) * 0.5;
      } else {
        decoOpacity = 0.5 + ((decoT - 0.66) / 0.34) * 0.5;
      }
    }
    decos.forEach((deco) => {
      deco.style.opacity = String(decoOpacity);
    });

    lines.forEach((line) => {
      const length = Number(line.dataset.length || 0);
      const offset = length * (1 - eased);
      line.style.strokeDashoffset = String(offset);
    });

    if (t < 1) {
      requestAnimationFrame(update);
      return;
    }

    const finalOpacity = isEntering ? '1' : '0';
    [...bgs, ...lines, ...decos].forEach((node) => {
      node.style.opacity = finalOpacity;
    });
    lines.forEach((line) => {
      line.style.strokeDasharray = '';
      line.style.strokeDashoffset = '';
    });
  };

  requestAnimationFrame(update);

  return {
    cancel: () => {
      cancelled = true;
    },
  };
};

export type { AnimateFrameAssemblerCompatOptions };
export { animateFrameAssemblerCompat };
