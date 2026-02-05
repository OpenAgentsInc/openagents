import { useEffect, useMemo, useRef } from 'react';

type Puff = {
  x: number;
  y: number;
  r: number;
  dx: number;
  dy: number;
  dr: number;
  phase: number;
};

interface HatcheryPuffsProps {
  color?: string;
  quantity?: number;
  className?: string;
}

const defaultColor = 'hsla(0, 0%, 100%, 0.2)';

export function HatcheryPuffs({
  color = defaultColor,
  quantity = 20,
  className,
}: HatcheryPuffsProps) {
  const canvasRef = useRef<HTMLCanvasElement>(null);
  const puffs = useMemo<Puff[]>(() => {
    return Array.from({ length: quantity }).map(() => ({
      x: Math.random(),
      y: Math.random(),
      r: 6 + Math.random() * 24,
      dx: (Math.random() - 0.5) * 0.01,
      dy: 0.04 + Math.random() * 0.06,
      dr: 4 + Math.random() * 10,
      phase: Math.random() * Math.PI * 2,
    }));
  }, [quantity]);

  useEffect(() => {
    const canvas = canvasRef.current;
    if (!canvas) {
      return;
    }
    const ctx = canvas.getContext('2d');
    if (!ctx) {
      return;
    }

    let rafId = 0;
    let cancelled = false;

    const resize = () => {
      const dpr = Math.min(window.devicePixelRatio || 1, 2);
      const { width, height } = canvas.getBoundingClientRect();
      const nextW = Math.max(1, Math.round(width * dpr));
      const nextH = Math.max(1, Math.round(height * dpr));
      if (canvas.width !== nextW || canvas.height !== nextH) {
        canvas.width = nextW;
        canvas.height = nextH;
      }
      ctx.setTransform(dpr, 0, 0, dpr, 0, 0);
    };

      const draw = (time: number) => {
      if (cancelled) {
        return;
      }
      resize();
      const { width, height } = canvas.getBoundingClientRect();
      ctx.clearRect(0, 0, width, height);

      puffs.forEach((puff, index) => {
        const t = time * 0.0002 + puff.phase + index * 0.2;
        const progress = (Math.sin(t) + 1) / 2;
          const x = (puff.x + puff.dx * t + 1) % 1;
          const y = (puff.y - puff.dy * t + 1) % 1;
        const r = puff.r + puff.dr * progress;
        const cx = x * width;
        const cy = y * height;

        const grd = ctx.createRadialGradient(cx, cy, 0, cx, cy, r);
        grd.addColorStop(0, color);
        grd.addColorStop(1, 'transparent');
        ctx.beginPath();
        ctx.fillStyle = grd;
        ctx.globalAlpha = 0.6 + 0.4 * progress;
        ctx.arc(cx, cy, r, 0, Math.PI * 2);
        ctx.fill();
        ctx.closePath();
      });

      rafId = window.requestAnimationFrame(draw);
    };

    rafId = window.requestAnimationFrame(draw);

    return () => {
      cancelled = true;
      if (rafId) {
        window.cancelAnimationFrame(rafId);
      }
    };
  }, [color, puffs]);

  return (
    <canvas
      ref={canvasRef}
      className={className}
      style={{ position: 'absolute', inset: 0, width: '100%', height: '100%' }}
    />
  );
}
