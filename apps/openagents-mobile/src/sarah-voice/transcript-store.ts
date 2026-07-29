export const SARAH_VOICE_TRANSCRIPT_SCHEMA =
  "openagents.mobile.sarah.voice-transcript.v1" as const;

export type SarahVoiceTranscriptRecord = Readonly<{
  schema: typeof SARAH_VOICE_TRANSCRIPT_SCHEMA;
  recordedAt: string;
  sessionRef: string;
  threadRef: string;
  utteranceRef: string;
  source: "user" | "assistant" | "tool";
  text: string;
}>;

export type SarahVoiceTranscriptFile = Readonly<{
  append: (line: string) => Promise<void>;
}>;

export const makeSarahVoiceTranscriptStore = (file: SarahVoiceTranscriptFile) => ({
  append: (record: Omit<SarahVoiceTranscriptRecord, "schema">): Promise<void> =>
    file.append(`${JSON.stringify({ schema: SARAH_VOICE_TRANSCRIPT_SCHEMA, ...record })}\n`),
});
