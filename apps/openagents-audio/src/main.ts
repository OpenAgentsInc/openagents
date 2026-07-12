import { createGoogleSttAdapter } from "./stt"
import { startAudioServer } from "./server"
import { createGoogleChirpTtsAdapter, DEFAULT_CHIRP_VOICE } from "./tts"
import { createProductionRetentionRuntime } from "./runtime-retention"
const required = (name: string): string => { const value = process.env[name]; if (!value) throw new Error(`openagents-audio: missing ${name}`); return value }
const adapter = await createGoogleSttAdapter({ projectId: required("GOOGLE_CLOUD_PROJECT"), location: process.env["OPENAGENTS_AUDIO_STT_LOCATION"] ?? "us", languageCode: process.env["OPENAGENTS_AUDIO_LANGUAGE"] ?? "en-US" })
const retention = await createProductionRetentionRuntime({ bucket: required("OPENAGENTS_AUDIO_RETENTION_BUCKET"), databaseUrl: required("OPENAGENTS_AUDIO_DATABASE_URL"), encryptionKeyBase64: required("OPENAGENTS_AUDIO_ENCRYPTION_KEY_BASE64"), keyEpoch: required("OPENAGENTS_AUDIO_KEY_EPOCH") })
const running = startAudioServer({ tokenSecret: required("OPENAGENTS_AUDIO_TOKEN_SECRET"), adapter, retention, tts: createGoogleChirpTtsAdapter(), ttsVoiceRef: process.env["OPENAGENTS_AUDIO_TTS_VOICE"] ?? DEFAULT_CHIRP_VOICE, port: Number(process.env["PORT"] ?? 8080), log: (event) => console.log(JSON.stringify(event)) })
const stop = () => running.stop().finally(() => process.exit(0)); process.on("SIGTERM", stop); process.on("SIGINT", stop)
