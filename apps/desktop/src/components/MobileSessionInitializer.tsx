import { useEffect, useState } from 'react';
import { useQuery } from 'convex/react';
import { api } from '../convex/_generated/api';

interface MobileSessionInitializerProps {
  mobileSessionId: string;
  localSessionId: string;
  onInitialMessageSent: () => void;
  sendMessage: (sessionId: string, message: string) => Promise<void>;
}

export function MobileSessionInitializer({ 
  mobileSessionId, 
  localSessionId, 
  onInitialMessageSent,
  sendMessage
}: MobileSessionInitializerProps) {
  const [hasSentInitialMessage, setHasSentInitialMessage] = useState(false);
  
  // Query messages from the mobile session
  const mobileMessages = useQuery(api.claude.getSessionMessages, { 
    sessionId: mobileSessionId,
    limit: 10 // Get the first few messages
  });

  useEffect(() => {
    // Skip if we've already sent the initial message
    if (hasSentInitialMessage || !mobileMessages || mobileMessages.length === 0) {
      return;
    }

    // Find the first user message (which should be the initial message)
    const initialMessage = mobileMessages.find(msg => msg.messageType === 'user');
    
    if (initialMessage) {
      console.log('📨 [MOBILE-INIT] Found initial message from mobile session:', initialMessage.content);
      
      // Send the initial message to Claude Code
      sendMessage(localSessionId, initialMessage.content)
        .then(() => {
          console.log('✅ [MOBILE-INIT] Initial message sent successfully');
          setHasSentInitialMessage(true);
          onInitialMessageSent();
        })
        .catch((error) => {
          console.error('❌ [MOBILE-INIT] Failed to send initial message:', error);
        });
    } else {
      console.log('⚠️ [MOBILE-INIT] No initial user message found in mobile session');
      // Still mark as completed to prevent infinite retries
      setHasSentInitialMessage(true);
      onInitialMessageSent();
    }
  }, [mobileMessages, hasSentInitialMessage, localSessionId, mobileSessionId, sendMessage, onInitialMessageSent]);

  return null; // This component doesn't render anything
}