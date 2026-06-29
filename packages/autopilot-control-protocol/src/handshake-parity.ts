export type HandshakeSession = {
  sessionRef: string
  state: string
}

export type HandshakeParity = {
  inSync: boolean
  sharedRefs: string[]
  desktopOnly: string[]
  mobileOnly: string[]
  stateMismatches: string[]
}

export const checkHandshakeParity = (
  desktop: HandshakeSession[],
  mobile: HandshakeSession[],
): HandshakeParity => {
  const desktopByRef = new Map(desktop.map((session) => [session.sessionRef, session.state]))
  const mobileByRef = new Map(mobile.map((session) => [session.sessionRef, session.state]))

  const sharedRefs = desktop
    .map((session) => session.sessionRef)
    .filter((sessionRef) => mobileByRef.has(sessionRef))
  const desktopOnly = desktop
    .map((session) => session.sessionRef)
    .filter((sessionRef) => !mobileByRef.has(sessionRef))
  const mobileOnly = mobile
    .map((session) => session.sessionRef)
    .filter((sessionRef) => !desktopByRef.has(sessionRef))
  const stateMismatches = sharedRefs.flatMap((sessionRef) => {
    const desktopState = desktopByRef.get(sessionRef)
    const mobileState = mobileByRef.get(sessionRef)

    if (desktopState === mobileState) return []
    return [`${sessionRef}: desktop ${desktopState}, mobile ${mobileState}`]
  })

  return {
    inSync: desktopOnly.length === 0 && mobileOnly.length === 0 && stateMismatches.length === 0,
    sharedRefs,
    desktopOnly,
    mobileOnly,
    stateMismatches,
  }
}
