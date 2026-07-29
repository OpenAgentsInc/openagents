import { NativeModule } from "expo";

import type { RealtimeAudioEvents, RealtimeAudioPlaybackStatus } from "./ExpoRealtimeAudio.types";

const unavailable = (): never => {
  throw new Error("Realtime PCM playback is available only in the native OpenAgents app.");
};

class ExpoRealtimeAudioWeb extends NativeModule<RealtimeAudioEvents> {
  start = unavailable;
  enqueue = unavailable;
  flush = unavailable;
  stop = (): void => undefined;
  getStatus = (): RealtimeAudioPlaybackStatus => ({
    started: false,
    queuedFrames: 0,
    playedMilliseconds: 0,
    interrupted: false,
  });
  startMicrophone = unavailable;
  stopMicrophone = (): void => undefined;
  isMicrophoneStarted = (): boolean => false;
}

export default new ExpoRealtimeAudioWeb();
