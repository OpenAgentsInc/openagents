import { useContext, useEffect, useReducer } from 'react';
import { TinyvexContext } from './Provider';
import type { MessageRowTs } from 'tricoder/types';

type Status = 'idle' | 'connecting' | 'ready' | 'error';
type Live = { assistant: string; thought?: string };

type State = {
  status: Status;
  threadId?: string;
  history: MessageRowTs[];
  live: Live;
  debug?: unknown;
  error?: string;
};

type Action =
  | { type: 'resolved'; threadId: string }
  | { type: 'ready' }
  | { type: 'history'; rows: MessageRowTs[] }
  | { type: 'live'; ev: Live }
  | { type: 'debug'; d: unknown }
  | { type: 'error'; error: string };

function reducer(state: State, action: Action): State {
  switch (action.type) {
    case 'resolved':
      return { ...state, threadId: action.threadId };
    case 'ready':
      return { ...state, status: 'ready' };
    case 'history':
      return { ...state, history: action.rows };
    case 'live':
      return { ...state, live: { ...state.live, ...action.ev } };
    case 'debug':
      return { ...state, debug: action.d };
    case 'error':
      return { ...state, status: 'error', error: action.error };
    default:
      return state;
  }
}

export function useTinyvexThread({ idOrAlias }: { idOrAlias: string }) {
  const client = useContext(TinyvexContext);
  if (!client) throw new Error('TinyvexProvider missing');

  const [st, dispatch] = useReducer(reducer, {
    status: 'connecting' as Status,
    history: [] as MessageRowTs[],
    live: { assistant: '' } as Live,
  });

  useEffect(() => {
    let off = () => {};
    (async () => {
      try {
        const threadId = await client.init(idOrAlias);
        dispatch({ type: 'resolved', threadId });
        off = client.onEvents({
          history: (rows) => dispatch({ type: 'history', rows }),
          live: (ev) => dispatch({ type: 'live', ev }),
          debug: (d) => dispatch({ type: 'debug', d }),
        });
        dispatch({ type: 'ready' });
      } catch (e) {
        dispatch({ type: 'error', error: e instanceof Error ? e.message : String(e) });
      }
    })();
    return () => off();
  }, [client, idOrAlias]);

  const send = (text: string, opts?: { resumeId?: 'last'; provider?: string }) => client.send(text, opts);
  const refresh = () => st.threadId && (client as any)['queryHistory']?.(st.threadId);

  return {
    status: st.status,
    threadId: st.threadId,
    history: st.history,
    live: st.live,
    send,
    refresh,
    debug: st.debug,
  };
}

