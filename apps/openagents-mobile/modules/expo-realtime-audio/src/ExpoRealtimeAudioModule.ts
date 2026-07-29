import { NativeModule, requireNativeModule } from "expo";

import type { RealtimeAudioEvents, RealtimeAudioPlaybackStatus } from "./ExpoRealtimeAudio.types";

declare class NativeExpoRealtimeAudio extends NativeModule<RealtimeAudioEvents> {
  start(sampleRate: number): void;
  enqueue(pcm16Base64: string): number;
  flush(): void;
  stop(): void;
  getStatus(): RealtimeAudioPlaybackStatus;
  startMicrophone(sampleRate: number): void;
  stopMicrophone(): void;
  isMicrophoneStarted(): boolean;
}

export default requireNativeModule<NativeExpoRealtimeAudio>("ExpoRealtimeAudio");
