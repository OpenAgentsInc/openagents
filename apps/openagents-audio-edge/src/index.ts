import { handleAudioEdgeProxy } from '../../openagents.com/workers/api/src/audio-edge-proxy'

export default {
  fetch: (request: Request, env: Parameters<typeof handleAudioEdgeProxy>[1]) => handleAudioEdgeProxy(request, env),
}
