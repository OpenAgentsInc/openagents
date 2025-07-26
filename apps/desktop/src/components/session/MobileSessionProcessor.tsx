import React from 'react';
import { MobileSessionInitializer } from '../MobileSessionInitializer';

interface MobileSessionToInitialize {
  mobileSessionId: string;
  localSessionId: string;
}

interface MobileSessionProcessorProps {
  mobileSessionsToInitialize: MobileSessionToInitialize[];
  sendMessage: (sessionId: string, message: string) => Promise<void>;
  handleInitialMessageSent: (mobileSessionId: string) => void;
}

export const MobileSessionProcessor: React.FC<MobileSessionProcessorProps> = ({
  mobileSessionsToInitialize,
  sendMessage,
  handleInitialMessageSent,
}) => {
  console.log('📱 [MOBILE-PROCESSOR] Rendering with', mobileSessionsToInitialize.length, 'sessions to initialize');
  
  return (
    <>
      {mobileSessionsToInitialize.map(({ mobileSessionId, localSessionId }) => (
        <MobileSessionInitializer
          key={mobileSessionId}
          mobileSessionId={mobileSessionId}
          localSessionId={localSessionId}
          onInitialMessageSent={() => handleInitialMessageSent(mobileSessionId)}
          sendMessage={sendMessage}
        />
      ))}
    </>
  );
};