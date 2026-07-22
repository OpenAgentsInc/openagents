export {
  decodeIdePortableClientSnapshot,
  emptyIdePortableClientSnapshot,
  type IdePortableClientSnapshot,
} from "../../ide/portable-client-contract.ts"

export type IdePortableRendererHost = Readonly<{
  snapshot: () => Promise<unknown>
  command: (value: unknown) => Promise<unknown>
}>

export const unavailableIdePortableRendererHost: IdePortableRendererHost = {
  snapshot: async () => ({
    status: { phase: "unavailable", cursor: null, pendingCommandCount: 0 },
    sessions: [],
    targetDirectories: [],
    attachments: [],
    commands: [],
    issues: [],
  }),
  command: async () => ({ _tag: "Refused", reason: "unavailable" }),
}
