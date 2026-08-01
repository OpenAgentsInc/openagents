type ParticipantSelectionSession = Readonly<{
  _roomIO?: Readonly<{
    setParticipant?: (participantRef: string | null) => void;
  }> | null;
}>;

export const selectSarahFloorParticipant = (
  session: unknown,
  participantRef: string | null,
): void => {
  const roomIO = (session as ParticipantSelectionSession)._roomIO;
  if (roomIO === undefined || roomIO === null || typeof roomIO.setParticipant !== "function") {
    throw new Error("The pinned LiveKit participant-selection boundary is unavailable");
  }
  roomIO.setParticipant(participantRef);
};
