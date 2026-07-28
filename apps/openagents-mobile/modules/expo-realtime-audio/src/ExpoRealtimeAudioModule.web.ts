import type {
  ExpoRealtimeAudioModule,
  RealtimeAudioPlaybackStatus,
} from "./ExpoRealtimeAudio.types";

const unavailable = (): never => {
  throw new Error("Realtime PCM playback is available only in the native OpenAgents app.");
};

const module: ExpoRealtimeAudioModule = {
  start: unavailable,
  enqueue: unavailable,
  flush: unavailable,
  stop: () => undefined,
  getStatus: (): RealtimeAudioPlaybackStatus => ({
    started: false,
    queuedFrames: 0,
    playedMilliseconds: 0,
    interrupted: false,
  }),
};

export default module;
