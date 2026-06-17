import * as Three from 'three'

import {
  type AnimationHandle,
  makeAnimationView,
  seededUnit,
  webglCanvas,
} from './element'

type Disposable = Readonly<{ dispose: () => void }>

type SceneContext = Readonly<{
  root: Three.Group
  disposables: Array<Disposable>
}>

type AnimationStep = (time: number, delta: number) => void
type SceneSetup = (context: SceneContext) => AnimationStep

const colors = {
  accent: 0xffb400,
  blue: 0x2979ff,
  cyan: 0xd6f6ff,
  green: 0x00c853,
  orange: 0xff6f00,
  red: 0xd32f2f,
  white: 0xf1efe8,
}

const vector = (x: number, y: number, z = 0): Three.Vector3 =>
  new Three.Vector3(x, y, z)

const disposeWith = (
  context: SceneContext,
  ...items: Array<Disposable>
): void => {
  context.disposables.push(...items)
}

const line = (
  context: SceneContext,
  points: ReadonlyArray<Three.Vector3>,
  color: number,
  opacity: number,
): Three.Line => {
  const geometry = new Three.BufferGeometry().setFromPoints([...points])
  const material = new Three.LineBasicMaterial({
    blending: Three.AdditiveBlending,
    color,
    opacity,
    transparent: true,
  })
  const object = new Three.Line(geometry, material)
  context.root.add(object)
  disposeWith(context, geometry, material)
  return object
}

const node = (
  context: SceneContext,
  position: Three.Vector3,
  color: number,
  radius: number,
): Three.Mesh => {
  const geometry = new Three.SphereGeometry(radius, 32, 16)
  const material = new Three.MeshBasicMaterial({
    blending: Three.AdditiveBlending,
    color,
    opacity: 0.82,
    transparent: true,
  })
  const mesh = new Three.Mesh(geometry, material)
  mesh.position.copy(position)
  context.root.add(mesh)
  disposeWith(context, geometry, material)

  const ringGeometry = new Three.TorusGeometry(
    radius * 1.45,
    radius * 0.035,
    10,
    72,
  )
  const ringMaterial = new Three.MeshBasicMaterial({
    blending: Three.AdditiveBlending,
    color,
    opacity: 0.55,
    transparent: true,
  })
  const ring = new Three.Mesh(ringGeometry, ringMaterial)
  ring.position.copy(position)
  context.root.add(ring)
  disposeWith(context, ringGeometry, ringMaterial)

  return mesh
}

const label = (
  context: SceneContext,
  text: string,
  position: Three.Vector3,
  color: string,
  scale = 0.52,
): Three.Sprite => {
  const canvas = document.createElement('canvas')
  canvas.width = 512
  canvas.height = 128
  const context2d = canvas.getContext('2d')
  if (context2d !== null) {
    context2d.clearRect(0, 0, canvas.width, canvas.height)
    context2d.font = '700 42px ui-monospace, SFMono-Regular, Menlo, monospace'
    context2d.textAlign = 'center'
    context2d.textBaseline = 'middle'
    context2d.fillStyle = color
    context2d.fillText(text, canvas.width / 2, canvas.height / 2)
  }

  const texture = new Three.CanvasTexture(canvas)
  const material = new Three.SpriteMaterial({
    map: texture,
    opacity: 0.9,
    transparent: true,
  })
  const sprite = new Three.Sprite(material)
  sprite.position.copy(position)
  sprite.scale.set(scale * 2.6, scale * 0.65, 1)
  context.root.add(sprite)
  disposeWith(context, texture, material)
  return sprite
}

const pulsePoint = (
  context: SceneContext,
  color: number,
  radius: number,
): Three.Mesh => {
  const geometry = new Three.SphereGeometry(radius, 20, 10)
  const material = new Three.MeshBasicMaterial({
    blending: Three.AdditiveBlending,
    color,
    opacity: 1,
    transparent: true,
  })
  const mesh = new Three.Mesh(geometry, material)
  context.root.add(mesh)
  disposeWith(context, geometry, material)
  return mesh
}

const makeGrammarScene =
  (setup: SceneSetup, cameraZ = 7.2) =>
  (element: HTMLElement): AnimationHandle => {
    const { canvas, size } = webglCanvas(element)
    const renderer = new Three.WebGLRenderer({
      alpha: true,
      antialias: true,
      canvas,
    })
    renderer.setClearColor(0x000000, 0)

    const scene = new Three.Scene()
    const camera = new Three.PerspectiveCamera(45, 1, 0.1, 100)
    camera.position.z = cameraZ

    const root = new Three.Group()
    scene.add(root)

    const disposables: Array<Disposable> = []
    const step = setup({ root, disposables })

    const resize = (): void => {
      const { height, width } = size()
      renderer.setPixelRatio(Math.min(window.devicePixelRatio || 1, 2))
      renderer.setSize(width, height, false)
      camera.aspect = width / height
      camera.updateProjectionMatrix()
    }

    let disposed = false
    let frame = 0
    let last = 0
    const tick = (time: number): void => {
      if (disposed) return
      const delta = last === 0 ? 0.016 : Math.min(0.05, (time - last) / 1000)
      last = time
      step(time, delta)
      renderer.render(scene, camera)
      frame = requestAnimationFrame(tick)
    }

    const observer =
      typeof ResizeObserver === 'undefined'
        ? null
        : new ResizeObserver(() => resize())
    resize()
    observer?.observe(element)
    frame = requestAnimationFrame(tick)

    return {
      dispose: () => {
        if (disposed) return
        disposed = true
        cancelAnimationFrame(frame)
        observer?.disconnect()
        disposables.forEach(disposable => disposable.dispose())
        renderer.dispose()
        element.replaceChildren()
      },
    }
  }

const contributorNodeSetup: SceneSetup = context => {
  const center = node(context, vector(0, 0, 0), colors.blue, 0.42)
  label(context, 'contributor node', vector(0, -1.05, 0.1), '#d6f6ff', 0.48)

  const contributors = Array.from({ length: 8 }, (_, index) => {
    const angle = (index / 8) * Math.PI * 2
    const radius = index % 2 === 0 ? 2.08 : 1.55
    const position = vector(Math.cos(angle) * radius, Math.sin(angle) * radius)
    line(context, [position, vector(0, 0, 0)], colors.blue, 0.22)
    const mesh = node(
      context,
      position,
      index % 3 === 0
        ? colors.green
        : index % 3 === 1
          ? colors.cyan
          : colors.accent,
      0.16,
    )
    label(
      context,
      index % 3 === 0 ? 'proof' : `P${index + 1}`,
      position.clone().add(vector(0, -0.34, 0.1)),
      '#f1efe8',
      0.26,
    )
    return { angle, mesh, radius }
  })

  return time => {
    const t = time * 0.001
    center.scale.setScalar(1 + Math.sin(t * 2.2) * 0.08)
    contributors.forEach((item, index) => {
      const angle = item.angle + t * 0.18
      item.mesh.position.set(
        Math.cos(angle) * item.radius,
        Math.sin(angle) * item.radius,
        Math.sin(t + index) * 0.08,
      )
    })
    context.root.rotation.z = Math.sin(t * 0.2) * 0.08
  }
}

const traceStrandSetup: SceneSetup = context => {
  const curve = new Three.CatmullRomCurve3([
    vector(-3.1, -0.85),
    vector(-2.0, 0.75),
    vector(-0.55, -0.2),
    vector(0.9, 0.9),
    vector(2.9, -0.5),
  ])
  line(context, curve.getPoints(80), colors.cyan, 0.35)
  ;[0, 0.24, 0.5, 0.74, 1].forEach((phase, index) => {
    const point = curve.getPoint(phase)
    node(
      context,
      point,
      index === 4 ? colors.green : index === 0 ? colors.white : colors.blue,
      0.18,
    )
  })
  label(context, 'trace strand', vector(0, -1.72, 0.1), '#d6f6ff', 0.55)
  const pulses = Array.from({ length: 5 }, (_, index) => ({
    mesh: pulsePoint(
      context,
      index % 2 === 0 ? colors.accent : colors.cyan,
      0.08,
    ),
    phase: seededUnit(index, 7),
    speed: 0.08 + seededUnit(index, 8) * 0.12,
  }))

  return (_, delta) => {
    pulses.forEach(pulse => {
      pulse.phase = (pulse.phase + pulse.speed * delta) % 1
      pulse.mesh.position.copy(curve.getPoint(pulse.phase))
    })
  }
}

const replayPairSetup: SceneSetup = context => {
  const worker = node(context, vector(-2.05, 0, 0), colors.blue, 0.44)
  const verifier = node(context, vector(2.05, 0, 0), colors.green, 0.44)
  label(context, 'worker', vector(-2.05, -0.75, 0.1), '#d6f6ff', 0.38)
  label(context, 'verifier', vector(2.05, -0.75, 0.1), '#d8ffe4', 0.38)
  const upper = new Three.QuadraticBezierCurve3(
    vector(-1.65, 0.18),
    vector(0, 1.28),
    vector(1.65, 0.18),
  )
  const lower = new Three.QuadraticBezierCurve3(
    vector(-1.65, -0.18),
    vector(0, -1.18),
    vector(1.65, -0.18),
  )
  line(context, upper.getPoints(56), colors.cyan, 0.42)
  line(context, lower.getPoints(56), colors.accent, 0.34)
  label(context, 'replay pair', vector(0, 1.7, 0.1), '#f1efe8', 0.52)

  const pulses = [
    { curve: upper, mesh: pulsePoint(context, colors.cyan, 0.1), phase: 0 },
    {
      curve: lower,
      mesh: pulsePoint(context, colors.accent, 0.1),
      phase: 0.48,
    },
  ]

  return (time, delta) => {
    const t = time * 0.001
    worker.scale.setScalar(1 + Math.sin(t * 2.6) * 0.06)
    verifier.scale.setScalar(1 + Math.cos(t * 2.2) * 0.06)
    pulses.forEach(pulse => {
      pulse.phase = (pulse.phase + delta * 0.18) % 1
      pulse.mesh.position.copy(pulse.curve.getPoint(pulse.phase))
    })
  }
}

const receiptBurstSetup: SceneSetup = context => {
  const hub = node(context, vector(0, 0, 0), colors.green, 0.34)
  label(context, 'receipt burst', vector(0, -1.55, 0.1), '#d8ffe4', 0.52)
  const rings = [0.75, 1.25, 1.75].map((radius, index) => {
    const geometry = new Three.TorusGeometry(radius, 0.012, 8, 128)
    const material = new Three.MeshBasicMaterial({
      blending: Three.AdditiveBlending,
      color: index === 1 ? colors.accent : colors.green,
      opacity: 0.55,
      transparent: true,
    })
    const mesh = new Three.Mesh(geometry, material)
    context.root.add(mesh)
    disposeWith(context, geometry, material)
    return { mesh, radius }
  })

  const shardGeometry = new Three.BufferGeometry()
  const shardPositions = new Float32Array(96 * 3)
  Array.from({ length: 96 }, (_, index) => {
    const angle = seededUnit(index, 1) * Math.PI * 2
    const radius = 0.45 + seededUnit(index, 2) * 2.5
    shardPositions[index * 3] = Math.cos(angle) * radius
    shardPositions[index * 3 + 1] = Math.sin(angle) * radius
    shardPositions[index * 3 + 2] = (seededUnit(index, 3) - 0.5) * 0.7
  })
  shardGeometry.setAttribute(
    'position',
    new Three.BufferAttribute(shardPositions, 3),
  )
  const shardMaterial = new Three.PointsMaterial({
    blending: Three.AdditiveBlending,
    color: colors.cyan,
    opacity: 0.75,
    size: 0.045,
    transparent: true,
  })
  const shards = new Three.Points(shardGeometry, shardMaterial)
  context.root.add(shards)
  disposeWith(context, shardGeometry, shardMaterial)

  return time => {
    const t = time * 0.001
    hub.scale.setScalar(1 + Math.sin(t * 3.4) * 0.1)
    rings.forEach((ring, index) => {
      const pulse = 1 + ((t * 0.24 + index * 0.22) % 1)
      ring.mesh.scale.setScalar(pulse)
      ;(ring.mesh.material as Three.MeshBasicMaterial).opacity =
        0.62 * (2 - pulse)
    })
    shards.rotation.z = t * 0.16
  }
}

const corpusAccretionSetup: SceneSetup = context => {
  label(context, 'corpus accretion', vector(0, -2.0, 0.1), '#d6f6ff', 0.52)
  const cubes = Array.from({ length: 40 }, (_, index) => {
    const x = (index % 10) * 0.52 - 2.34
    const y = Math.floor(index / 10) * 0.46 - 0.7
    const geometry = new Three.BoxGeometry(0.34, 0.22, 0.08)
    const material = new Three.MeshBasicMaterial({
      blending: Three.AdditiveBlending,
      color:
        index < 25 ? colors.blue : index < 34 ? colors.green : colors.accent,
      opacity: index < 25 ? 0.55 : 0.82,
      transparent: true,
      wireframe: index >= 34,
    })
    const mesh = new Three.Mesh(geometry, material)
    mesh.position.set(x, y, (seededUnit(index, 5) - 0.5) * 0.3)
    context.root.add(mesh)
    disposeWith(context, geometry, material)
    return { index, mesh }
  })

  return time => {
    const t = time * 0.001
    cubes.forEach(({ index, mesh }) => {
      mesh.position.z = Math.sin(t * 1.2 + index * 0.4) * 0.18
      mesh.rotation.z = Math.sin(t * 0.6 + index) * 0.04
    })
    context.root.rotation.x = Math.sin(t * 0.18) * 0.16
  }
}

const quarantineWindowSetup: SceneSetup = context => {
  line(context, [vector(-3, 0.72), vector(2.85, 0.72)], colors.green, 0.42)
  line(context, [vector(-2.3, -0.8), vector(1.8, -0.8)], colors.orange, 0.48)
  ;[-2.8, -1.25, 0.25, 1.75].forEach((x, index) => {
    node(
      context,
      vector(x, 0.72),
      index === 3 ? colors.green : colors.blue,
      0.16,
    )
  })
  const held = node(context, vector(-0.3, -0.8), colors.orange, 0.28)
  const gate = node(context, vector(2.65, 0.72), colors.cyan, 0.24)
  line(context, [vector(1.8, -0.8), vector(2.65, 0.72)], colors.orange, 0.24)
  label(context, 'quarantine window', vector(0, -1.65, 0.1), '#ffe0c2', 0.5)
  label(context, 'accepted', vector(-2.35, 1.18, 0.1), '#d8ffe4', 0.32)
  label(context, 'held', vector(-0.3, -1.25, 0.1), '#ffe0c2', 0.32)

  return time => {
    const t = time * 0.001
    held.scale.setScalar(1 + Math.sin(t * 3) * 0.12)
    gate.rotation.z = t * 0.7
  }
}

const energyOutcomeMeterSetup: SceneSetup = context => {
  label(context, 'energy outcome meter', vector(0, -1.92, 0.1), '#f1efe8', 0.5)
  const specs = [
    { color: colors.blue, label: 'compute', width: 1.25, x: -2.15 },
    { color: colors.green, label: 'verified work', width: 1.85, x: -0.32 },
    { color: colors.accent, label: 'settlement', width: 1.05, x: 1.72 },
  ]
  const bars = specs.map((spec, index) => {
    const geometry = new Three.BoxGeometry(spec.width, 0.48, 0.18)
    const material = new Three.MeshBasicMaterial({
      blending: Three.AdditiveBlending,
      color: spec.color,
      opacity: 0.72,
      transparent: true,
    })
    const mesh = new Three.Mesh(geometry, material)
    mesh.position.set(spec.x, 0.2, 0)
    context.root.add(mesh)
    label(
      context,
      spec.label,
      vector(spec.x, -0.34, 0.1),
      '#f1efe8',
      index === 1 ? 0.34 : 0.3,
    )
    disposeWith(context, geometry, material)
    return { index, mesh }
  })
  line(context, [vector(-2.8, 0.92), vector(2.45, 0.92)], colors.cyan, 0.3)

  return time => {
    const t = time * 0.001
    bars.forEach(({ index, mesh }) => {
      mesh.scale.y = 1 + Math.sin(t * 1.8 + index * 0.9) * 0.18
      mesh.position.z = Math.sin(t * 1.1 + index) * 0.08
    })
  }
}

const proofDrawerSetup: SceneSetup = context => {
  label(context, 'proof drawer', vector(0, -2.0, 0.1), '#d6f6ff', 0.54)
  const planes = ['window', 'replay', 'receipt', 'settle'].map(
    (name, index) => {
      const geometry = new Three.PlaneGeometry(2.35, 0.52)
      const material = new Three.MeshBasicMaterial({
        blending: Three.AdditiveBlending,
        color:
          index === 0
            ? colors.blue
            : index === 1
              ? colors.green
              : index === 2
                ? colors.accent
                : colors.cyan,
        opacity: 0.22 + index * 0.08,
        side: Three.DoubleSide,
        transparent: true,
      })
      const mesh = new Three.Mesh(geometry, material)
      mesh.position.set(-0.2 + index * 0.28, 1.05 - index * 0.58, -index * 0.18)
      mesh.rotation.y = -0.18
      context.root.add(mesh)
      label(
        context,
        name,
        mesh.position.clone().add(vector(0, 0.02, 0.18)),
        '#f1efe8',
        0.32,
      )
      disposeWith(context, geometry, material)
      return mesh
    },
  )
  planes.slice(0, -1).forEach((plane, index) => {
    const next = planes[index + 1]
    if (next !== undefined) {
      line(
        context,
        [plane.position.clone(), next.position.clone()],
        colors.white,
        0.18,
      )
    }
  })

  return time => {
    const t = time * 0.001
    planes.forEach((plane, index) => {
      plane.rotation.z = Math.sin(t * 0.6 + index * 0.4) * 0.035
    })
    context.root.rotation.y = Math.sin(t * 0.22) * 0.12
  }
}

export const trainingContributorNodeView = makeAnimationView(
  'oa-training-grammar-contributor-node',
  makeGrammarScene(contributorNodeSetup),
)

export const trainingTraceStrandView = makeAnimationView(
  'oa-training-grammar-trace-strand',
  makeGrammarScene(traceStrandSetup),
)

export const trainingReplayPairView = makeAnimationView(
  'oa-training-grammar-replay-pair',
  makeGrammarScene(replayPairSetup),
)

export const trainingReceiptBurstView = makeAnimationView(
  'oa-training-grammar-receipt-burst',
  makeGrammarScene(receiptBurstSetup),
)

export const trainingCorpusAccretionView = makeAnimationView(
  'oa-training-grammar-corpus-accretion',
  makeGrammarScene(corpusAccretionSetup),
)

export const trainingQuarantineWindowView = makeAnimationView(
  'oa-training-grammar-quarantine-window',
  makeGrammarScene(quarantineWindowSetup),
)

export const trainingEnergyOutcomeMeterView = makeAnimationView(
  'oa-training-grammar-energy-outcome-meter',
  makeGrammarScene(energyOutcomeMeterSetup),
)

export const trainingProofDrawerView = makeAnimationView(
  'oa-training-grammar-proof-drawer',
  makeGrammarScene(proofDrawerSetup),
)
