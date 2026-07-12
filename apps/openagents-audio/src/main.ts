import { createGoogleSttAdapter } from "./stt"
import { startAudioServer } from "./server"
const required = (name: string): string => { const value = process.env[name]; if (!value) throw new Error(`openagents-audio: missing ${name}`); return value }
const adapter = await createGoogleSttAdapter({ projectId: required("GOOGLE_CLOUD_PROJECT"), location: process.env["OPENAGENTS_AUDIO_STT_LOCATION"] ?? "us", languageCode: process.env["OPENAGENTS_AUDIO_LANGUAGE"] ?? "en-US" })
const running = startAudioServer({ tokenSecret: required("OPENAGENTS_AUDIO_TOKEN_SECRET"), adapter, port: Number(process.env["PORT"] ?? 8080), log: (event) => console.log(JSON.stringify(event)) })
const stop = () => running.stop().finally(() => process.exit(0)); process.on("SIGTERM", stop); process.on("SIGINT", stop)
