/**
 * Host interaction is separate from fleet state. A decoded owner projection
 * makes supervision data visible; it never proves that this renderer can
 * submit commands or that the server will authorize them.
 */
export type SarahOwnerFleetInteractionMode = "read_only" | "interactive"

export const SARAH_OWNER_FLEET_READ_ONLY: SarahOwnerFleetInteractionMode =
  "read_only"
export const SARAH_OWNER_FLEET_INTERACTIVE: SarahOwnerFleetInteractionMode =
  "interactive"
