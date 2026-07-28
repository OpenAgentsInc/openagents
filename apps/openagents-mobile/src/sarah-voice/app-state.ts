export type SarahVoiceAppStateAction = "resume" | "stop_audio" | "end_session"

export const sarahVoiceAppStateAction = (
  state: "active" | "background" | "inactive" | "unknown" | "extension",
): SarahVoiceAppStateAction => {
  if (state === "active") return "resume"
  if (state === "background") return "end_session"
  return "stop_audio"
}
