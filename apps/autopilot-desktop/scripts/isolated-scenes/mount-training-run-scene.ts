import {
  registerTrainingRunElement,
  trainingRunTagName,
} from "@openagentsinc/three-effect/foldkit"
import type { TrainingRunVisualizationOptions } from "@openagentsinc/three-effect/core"

registerTrainingRunElement()

export type MountedIsolatedScene = Readonly<{
  mounted: boolean
  sceneName: string
  beamCount: number
  entityCount: number
  nodeCount: number
}>

export const mountTrainingRunIsolatedScene = (
  sceneName: string,
  visualization: TrainingRunVisualizationOptions,
): MountedIsolatedScene => {
  const mount = document.getElementById("scene")
  if (mount === null) throw new Error("missing #scene mount")

  const host = document.createElement(trainingRunTagName) as HTMLElement & {
    visualization: TrainingRunVisualizationOptions
  }
  host.id = `${sceneName}-scene`
  host.style.display = "block"
  host.style.width = "960px"
  host.style.height = "540px"
  host.style.minHeight = "540px"
  host.visualization = visualization
  mount.append(host)

  const mounted = {
    mounted: true,
    sceneName,
    beamCount: visualization.beams?.length ?? 0,
    entityCount: visualization.entities?.length ?? 0,
    nodeCount: visualization.nodes?.length ?? 0,
  }
  ;(globalThis as unknown as { __isolatedScene?: MountedIsolatedScene }).__isolatedScene =
    mounted
  return mounted
}
