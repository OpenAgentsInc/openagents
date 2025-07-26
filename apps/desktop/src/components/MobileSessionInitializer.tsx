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
  console.log('🚀 [MOBILE-INIT] Component mounted', { mobileSessionId, localSessionId });
  
  const [hasSentInitialMessage, setHasSentInitialMessage] = useState(false);
  const [isSending, setIsSending] = useState(false);
  
  // Query messages from the mobile session
  const mobileMessages = useQuery(api.claude.getSessionMessages, { 
    sessionId: mobileSessionId,
    limit: 10 // Get the first few messages
  });

  useEffect(() => {
    console.log('🔍 [MOBILE-INIT] Checking for initial message...', {
      mobileSessionId,
      localSessionId,
      hasSentInitialMessage,
      isSending,
      messagesLength: mobileMessages?.length || 0
    });
    
    // Skip if we've already sent the initial message or are currently sending
    if (hasSentInitialMessage || isSending || !mobileMessages || mobileMessages.length === 0) {
      return;
    }

    // Find the first user message (which should be the initial message)
    const initialMessage = mobileMessages.find((msg: any) => msg.messageType === 'user');
    
    if (initialMessage) {
      console.log('📨 [MOBILE-INIT] Found initial message from mobile session:', initialMessage.content);
      
      // Immediately mark as sending to prevent re-triggers
      setIsSending(true);
      
      // Send the initial message to Claude Code
      sendMessage(localSessionId, initialMessage.content)
        .then(() => {
          console.log('✅ [MOBILE-INIT] Initial message sent successfully');
          setHasSentInitialMessage(true);
          onInitialMessageSent();
        })
        .catch((error) => {
          console.error('❌ [MOBILE-INIT] Failed to send initial message:', error);
          setIsSending(false); // Reset on error to allow retry
        });
    } else {
      console.log('⚠️ [MOBILE-INIT] No initial user message found in mobile session');
      // Still mark as completed to prevent infinite retries
      setHasSentInitialMessage(true);
      onInitialMessageSent();
    }
  }, [mobileMessages?.length]); // Only depend on messages length to avoid loops

  return null; // This component doesn't render anything
}