export type LogKind = 'md' | 'reason' | 'text' | 'json' | 'summary' | 'delta';

export type LogDetail = {
  id: number;
  text: string;
  kind: LogKind;
  deemphasize?: boolean;
  ts?: number;
};

const store = new Map<number, LogDetail>();

export function putLog(detail: LogDetail) {
  store.set(detail.id, detail);
}

export function getLog(id: number): LogDetail | undefined {
  return store.get(id);
}

