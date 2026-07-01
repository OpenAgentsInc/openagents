// Assembles the concrete on-device decider for the Khala Code desktop host.
// GPT-OSS is the launch default. Apple FM stays present in source, but it must
// be explicitly enabled so launch builds never boot or nudge the bridge.

import { createAppleFmSidecarHost } from "./apple-fm-sidecar.js"
import { createAppleFmDeciderBackend } from "./apple-fm-decider-backend.js"
import { createGptOssDeciderBackend } from "./gpt-oss-decider-backend.js"
import {
  createOnDeviceDecider,
  type OnDeviceDeciderBackend,
  type OnDeviceDecider,
  type OnDeviceDeciderPlatform,
} from "../shared/on-device-decider.js"

export type OnDeviceDeciderHostOptions = {
  readonly appleFmEnabled?: boolean
  readonly platform?: OnDeviceDeciderPlatform
  readonly env?: Readonly<Record<string, string | undefined>>
  readonly sidecar?: { readiness: ReturnType<typeof createAppleFmSidecarHost>["readiness"] }
}

export function createOnDeviceDeciderHost(
  options: OnDeviceDeciderHostOptions = {},
): OnDeviceDecider {
  const platform: OnDeviceDeciderPlatform = options.platform ?? {
    platform: process.platform,
    arch: process.arch,
  }
  const env = options.env ?? Bun.env
  const backends: Partial<Record<"apple_fm" | "gpt_oss", OnDeviceDeciderBackend>> = {
    gpt_oss: createGptOssDeciderBackend({ env }),
  }

  if (options.appleFmEnabled === true) {
    const sidecar = options.sidecar ?? createAppleFmSidecarHost({ env })
    backends.apple_fm = createAppleFmDeciderBackend({ sidecar })
  }

  return createOnDeviceDecider({
    platform,
    backends,
  })
}
