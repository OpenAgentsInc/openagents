// Assembles the concrete on-device decider for the Khala Code desktop host:
// the Apple FM backend (wired to the sidecar) + the GPT-OSS backend (from env),
// selected by the real process platform. Both are optional and fail soft.

import { createAppleFmSidecarHost } from "./apple-fm-sidecar.js"
import { createAppleFmDeciderBackend } from "./apple-fm-decider-backend.js"
import { createGptOssDeciderBackend } from "./gpt-oss-decider-backend.js"
import {
  createOnDeviceDecider,
  type OnDeviceDecider,
  type OnDeviceDeciderPlatform,
} from "../shared/on-device-decider.js"

export type OnDeviceDeciderHostOptions = {
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
  const sidecar = options.sidecar ?? createAppleFmSidecarHost({ env })

  return createOnDeviceDecider({
    platform,
    backends: {
      apple_fm: createAppleFmDeciderBackend({ sidecar }),
      gpt_oss: createGptOssDeciderBackend({ env }),
    },
  })
}
