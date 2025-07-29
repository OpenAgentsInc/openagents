import { useEffect, useState } from 'react';
import { useQuery } from 'convex/react';
import { api } from '../convex/_generated/api';

interface MobileSessionInitializerProps {
  mobileSessionId: string;
  localSessionId: string;
  onInitialMessageSent: () => void;
  sendMessage: (sessionId: string, message: string) => Promise<void>;
}

// Use a global flag to prevent multiple instances from processing the same session
const processingSessions = new Set<string>();

export function MobileSessionInitializer({ 
  mobileSessionId, 
  localSessionId, 
  onInitialMessageSent,
  sendMessage
}: MobileSessionInitializerProps) {
  console.log('🚀 [MOBILE-INIT] Component mounted', { mobileSessionId, localSessionId });
  
  // Check if another instance is already processing this session
  if (processingSessions.has(localSessionId)) {
    console.log('⚠️ [MOBILE-INIT] Another instance is already processing this session');
    return null;
  }
  
  const [hasSentInitialMessage, setHasSentInitialMessage] = useState(false);
  const [isSending, setIsSending] = useState(false);
  
  // Query messages from the mobile session
  const mobileMessages = useQuery(api.confect.mobile_sync.getSessionMessages, { 
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
      processingSessions.add(localSessionId);
      
      // Send the initial message to Claude Code
      console.log('🚀 [MOBILE-INIT] Calling sendMessage with:', {
        localSessionId,
        messageContent: initialMessage.content
      });
      
      try {
        const sendPromise = sendMessage(localSessionId, initialMessage.content);
        console.log('📤 [MOBILE-INIT] sendMessage called, awaiting promise...');
        
        sendPromise
          .then(() => {
            console.log('✅ [MOBILE-INIT] Initial message sent successfully');
            setHasSentInitialMessage(true);
            processingSessions.delete(localSessionId);
            onInitialMessageSent();
          })
          .catch((error) => {
            console.error('❌ [MOBILE-INIT] Failed to send initial message:', error);
            setIsSending(false); // Reset on error to allow retry
            processingSessions.delete(localSessionId);
          });
      } catch (syncError) {
        console.error('💥 [MOBILE-INIT] Synchronous error calling sendMessage:', syncError);
        setIsSending(false);
        processingSessions.delete(localSessionId);
      }
    } else {
      console.log('⚠️ [MOBILE-INIT] No initial user message found in mobile session');
      // Still mark as completed to prevent infinite retries
      setHasSentInitialMessage(true);
      onInitialMessageSent();
    }
  }, [mobileMessages?.length]); // Only depend on messages length to avoid loops

  // Cleanup on unmount
  useEffect(() => {
    return () => {
      processingSessions.delete(localSessionId);
    };
  }, [localSessionId]);

  return null; // This component doesn't render anything
}