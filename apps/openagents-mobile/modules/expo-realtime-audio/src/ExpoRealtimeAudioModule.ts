import { requireNativeModule } from "expo";

import type { ExpoRealtimeAudioModule } from "./ExpoRealtimeAudio.types";

export default requireNativeModule<ExpoRealtimeAudioModule>("ExpoRealtimeAudio");
