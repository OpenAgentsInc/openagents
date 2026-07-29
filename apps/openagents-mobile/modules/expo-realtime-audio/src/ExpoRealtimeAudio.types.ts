export type RealtimeAudioPlaybackStatus = Readonly<{
  started: boolean;
  queuedFrames: number;
  playedMilliseconds: number;
  interrupted: boolean;
}>;

export type RealtimeAudioMicrophoneBuffer = Readonly<{
  pcm16Base64: string;
  sampleRate: number;
  channels: number;
}>;

export type RealtimeAudioEvents = {
  onMicrophoneBuffer: (event: RealtimeAudioMicrophoneBuffer) => void;
};
