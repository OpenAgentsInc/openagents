import React from 'react';
import { SessionStreamManager } from '../SessionStreamManager';

interface Message {
  id: string;
  message_type: string;
  content: string;
  timestamp: string;
  tool_info?: {
    tool_name: string;
    tool_use_id: string;
    input: Record<string, any>;
    output?: string;
  };
}

interface Session {
  id: string;
  projectPath: string;
  messages: Message[];
  inputMessage: string;
  isLoading: boolean;
  isInitializing?: boolean;
}

interface SessionManagerProps {
  sessions: Session[];
  handleMessagesUpdate: (sessionId: string, messages: Message[]) => void;
  handleStreamError: (sessionId: string, error: Error) => void;
  sessionIdMapping: Map<string, string>; // Claude Code UUID -> Mobile session ID mapping
}

export const SessionManager: React.FC<SessionManagerProps> = ({
  sessions,
  handleMessagesUpdate,
  handleStreamError,
  sessionIdMapping,
}) => {
  return (
    <>
      {sessions.map(session => {
        // Get mobile session ID for persistence (fallback to session.id if not mapped)
        const persistToSessionId = sessionIdMapping.get(session.id) || session.id;
        
        return (
          <SessionStreamManager
            key={session.id}
            sessionId={session.id} // Claude Code UUID for streaming
            persistToSessionId={persistToSessionId} // Mobile session ID for persistence
            isInitializing={session.isInitializing || false}
            onMessagesUpdate={handleMessagesUpdate}
            onError={handleStreamError}
          />
        );
      })}
    </>
  );
};