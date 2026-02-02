export interface LogSession {
  id: string;
  startedAt: string;
  endedAt?: string;
  logs: string;
}

export async function getAllSessions(): Promise<LogSession[]> {
  return [];
}

export function isStorageAvailable(): boolean {
  return typeof indexedDB !== "undefined";
}
