import { useEffect, useState } from 'react';
import { createFileRoute } from '@tanstack/react-router';
import { useMutation, useQuery } from 'convex/react';
import { useAuth } from '@workos/authkit-tanstack-react-start/client';
import { api } from '../../../convex/_generated/api';
import { posthogCapture } from '@/lib/posthog';
import { OpenClawChatUI } from '@/components/openclaw/OpenClawChatUI';
import { HatcheryWaitlistOverlay } from '@/components/hatchery/HatcheryWaitlistOverlay';

export const Route = createFileRoute('/_openclaw/hatchery')({
  component: HatcheryPage,
  validateSearch: (search: Record<string, unknown>) => ({
    focus: typeof search.focus === 'string' ? search.focus : undefined,
  }),
});

function HatcheryPage() {
  const { user, loading: authLoading } = useAuth();
  const accessStatus = useQuery(api.access.getStatus);
  const [email, setEmail] = useState('');
  const [waitlistStatus, setWaitlistStatus] = useState<
    'idle' | 'submitting' | 'success' | 'error'
  >('idle');
  const [waitlistError, setWaitlistError] = useState<string | null>(null);
  const joinWaitlistMutation = useMutation(api.waitlist.joinWaitlist);

  const accessAllowed = accessStatus?.allowed === true;
  const waitlistEntry = accessStatus?.waitlistEntry ?? null;
  const waitlistApproved = accessStatus?.waitlistApproved === true;
  const overlayVisible = !accessAllowed;

  useEffect(() => {
    posthogCapture('hatchery_view');
  }, []);

  const handleWaitlistSubmit = async (trimmed: string) => {
    setWaitlistError(null);
    setWaitlistStatus('submitting');
    posthogCapture('hatchery_waitlist_submit', { source: 'hatchery' });
    try {
      const result = await joinWaitlistMutation({ email: trimmed, source: 'hatchery' });
      posthogCapture('hatchery_waitlist_success', { source: 'hatchery', joined: result.joined });
      setWaitlistStatus('success');
    } catch (err) {
      const message = err instanceof Error ? err.message : 'Something went wrong';
      posthogCapture('hatchery_waitlist_error', { source: 'hatchery', message });
      setWaitlistStatus('error');
      setWaitlistError(message);
    }
  };

  return (
    <div className="relative flex h-full min-h-0 w-full flex-col">
      <HatcheryWaitlistOverlay
        visible={overlayVisible}
        authLoading={authLoading}
        waitlistApproved={waitlistApproved}
        waitlistEntry={waitlistEntry}
        waitlistStatus={waitlistStatus}
        waitlistError={waitlistError}
        email={email}
        userEmail={user?.email ?? null}
        onEmailChange={setEmail}
        onSubmit={handleWaitlistSubmit}
      />
      <OpenClawChatUI />
    </div>
  );
}
