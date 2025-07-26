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
}

export const SessionManager: React.FC<SessionManagerProps> = ({
  sessions,
  handleMessagesUpdate,
  handleStreamError,
}) => {
  return (
    <>
      {sessions.map(session => (
        <SessionStreamManager
          key={session.id}
          sessionId={session.id}
          isInitializing={session.isInitializing || false}
          onMessagesUpdate={handleMessagesUpdate}
          onError={handleStreamError}
        />
      ))}
    </>
  );
};