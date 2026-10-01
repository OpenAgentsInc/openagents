export interface Session {
  userId: string;
  expiresAt: number;
}

export function owner(session: Session): string {
  return session.userID;
}
