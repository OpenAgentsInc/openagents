/** Original Quick MMORPG height fog, converted from source units at scale 1/6. */
export const MMORPG_FOG_COLOR = [93 / 255, 103 / 255, 158 / 255] as const;

export function mmorpgFogAmount(
  eyeY: number,
  delta: readonly number[],
): number {
  if (
    !Number.isFinite(eyeY) ||
    delta.length !== 3 ||
    !delta.every(Number.isFinite)
  )
    throw Error("Use a finite fog ray.");
  const d = Math.hypot(...delta),
    q = d * delta[1] * 0.00072;
  const ratio =
    Math.abs(q) < 0.001
      ? 1 - q / 2 + (q * q) / 6
      : -Math.expm1(Math.min(80, -q)) / q;
  return Math.max(
    0,
    Math.min(1, d * d * 0.000036 * Math.exp(-eyeY * 0.00012) * ratio),
  );
}

// The horizontal limit avoids the original GLSL's 0/0 without changing its fog law.
export const MMORPG_FOG_WGSL = `
const mmoFogColor=vec3(93.0/255.0,103.0/255.0,158.0/255.0);
fn mmoFog(y:f32,v:vec3<f32>)->f32{let d=length(v);let q=d*v.y*.00072;var r=1.0-q*.5+q*q/6.0;if abs(q)>.001{r=(1.0-exp(clamp(-q,-80.0,80.0)))/q;}return clamp(d*d*.000036*exp(-y*.00012)*r,0.0,1.0);}
`;
