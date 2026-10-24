export function gameplayActivity(feature: string, editor = false): any {
  const actors = Array.from({ length: 8 }, (_, id) => ({
    id,
    position: [id, 0, 0],
    yaw: 0,
    alive: true,
    deathSeconds: 0,
    animationTime: 0,
  }));
  const enemies = { total: 8, alive: 8, killed: 0, actors };
  const base = {
    enemies,
    renderer: { modelLoaded: true, submittedActors: 8 },
    revision: 0,
    player: [0, 0, 0],
    shaderIdentity: "a".repeat(64),
    gameplay: { castCount: 0, destroyedCount: 0 },
  };
  const phase = (name: string, atMs: number, extra = {}) =>
    structuredClone({ ...base, phase: name, atMs, ...extra });
  if (feature.startsWith("aim-")) {
    const sky = feature === "aim-sky";
    const expected =
      feature === "aim-third-locked"
        ? [0, 0.22]
        : feature === "aim-third-cursor"
          ? [0.2, 0.04]
          : [0, 0];
    return {
      feature,
      phases: [
        { phase: "real-input-aim-selected", atMs: 0, casts: 0, impacts: 0 },
        { phase: "cast-through-real-control-requested", atMs: 100 },
        {
          phase: sky ? "sky-cast-rejected" : "impact-matches-world-ring",
          atMs: 200,
          casts: sky ? 0 : 1,
          impacts: sky ? 0 : 1,
          locked: feature.endsWith("locked"),
          target: sky ? null : {},
          aimTriangles: sky ? 0 : 128,
        },
        { phase: "restored-after-real-aiming", atMs: 29900 },
      ],
      samples: sky ? [] : [{ actualNDC: expected, aimTriangles: 128 }],
    };
  }
  const start = phase("all-eight-alive", 0);
  if (feature === "enemies")
    return {
      feature,
      phases: [start, phase("patrol-window-complete", 30000)],
      samples: [
        { enemies },
        {
          enemies: {
            ...enemies,
            actors: actors.map((a) => ({ ...a, position: [a.id, 0, 2] })),
          },
        },
      ],
    };
  const restored = phase("all-eight-revived-and-damage-cleared", 29900);
  if (feature === "enemy-miss-cover")
    return {
      feature,
      phases: [
        start,
        phase("sky-miss-rejected", 100),
        phase("intact-arch-blocks-lightning", 1300, {
          gameplay: { castCount: 1, destroyedCount: 0 },
        }),
        restored,
      ],
      attempts: [],
    };
  const dead = {
    ...enemies,
    alive: 7,
    killed: 1,
    actors: actors.map((a) => (a.id === 6 ? { ...a, alive: false } : a)),
  };
  const held = {
    ...dead,
    actors: dead.actors.map((a) => (a.alive ? a : { ...a, deathSeconds: 2.2 })),
  };
  return {
    feature,
    phases: [
      start,
      phase("death-animation-started", 200, { enemies: dead }),
      phase("corpse-held-after-death-clip", 2400, { enemies: held }),
      ...(editor
        ? [
            phase("settled-corpses-before-failed-shader", 2500, {
              enemies: held,
            }),
            phase("failed-shader-retains-corpses", 2600, { enemies: held }),
          ]
        : []),
      restored,
    ],
    attempts: [{ ok: true }],
  };
}
