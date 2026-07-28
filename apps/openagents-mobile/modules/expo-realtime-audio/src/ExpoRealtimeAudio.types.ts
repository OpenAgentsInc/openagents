export type RealtimeAudioPlaybackStatus = Readonly<{
  started: boolean;
  queuedFrames: number;
  playedMilliseconds: number;
  interrupted: boolean;
}>;

export type ExpoRealtimeAudioModule = Readonly<{
  start: (sampleRate: number) => void;
  enqueue: (pcm16Base64: string) => number;
  flush: () => void;
  stop: () => void;
  getStatus: () => RealtimeAudioPlaybackStatus;
}>;
