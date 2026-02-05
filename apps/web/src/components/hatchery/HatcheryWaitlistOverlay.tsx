import { Card, CardContent } from '@/components/ui/card';
import { Button } from '@/components/ui/button';

const EMAIL_RE = /^[^\s@]+@[^\s@]+\.[^\s@]+$/;

export type WaitlistOverlayProps = {
  visible: boolean;
  authLoading: boolean;
  waitlistApproved: boolean;
  waitlistEntry: unknown;
  waitlistStatus: 'idle' | 'submitting' | 'success' | 'error';
  waitlistError: string | null;
  email: string;
  userEmail: string | null;
  onEmailChange: (value: string) => void;
  onSubmit: (email: string) => Promise<void>;
};

export function HatcheryWaitlistOverlay({
  visible,
  authLoading,
  waitlistApproved,
  waitlistEntry,
  waitlistStatus,
  waitlistError,
  email,
  userEmail,
  onEmailChange,
  onSubmit,
}: WaitlistOverlayProps) {
  if (!visible) return null;

  const handleSubmit = async (e: React.FormEvent) => {
    e.preventDefault();
    const trimmed = (userEmail ?? email).trim();
    if (!trimmed) return;
    if (!EMAIL_RE.test(trimmed)) return;
    await onSubmit(trimmed);
  };

  return (
    <>
      <div className="pointer-events-auto absolute inset-0 z-20 bg-black/80" aria-hidden />
      <div className="pointer-events-none absolute inset-0 z-20 flex items-center justify-center p-4">
        <Card className="pointer-events-auto w-full max-w-md border border-[#1a1525] bg-[#0d0a14]/95 px-10 py-8 shadow-xl ring-1 ring-[#252030]/50">
          <CardContent className="flex flex-col items-center gap-6 p-0 text-center">
            {authLoading ? (
              <div className="flex flex-col items-center gap-3">
                <span className="font-square721 text-xl font-medium text-zinc-100">
                  Checking access…
                </span>
                <span className="font-square721 text-base text-zinc-300">
                  Hang tight while we load your status.
                </span>
              </div>
            ) : waitlistApproved || waitlistStatus === 'success' || waitlistEntry ? (
              <div className="flex flex-col items-center gap-3">
                <span className="font-square721 text-xl font-medium text-zinc-100">
                  You're on the list
                </span>
                <span className="font-square721 text-base text-zinc-300">
                  Thanks! We'll email you as soon as access opens.
                </span>
              </div>
            ) : (
              <>
                <div className="flex flex-col items-center gap-3">
                  <span className="font-square721 text-xl font-medium text-zinc-100">
                    Coming Soon: The Hatchery
                  </span>
                  <span className="font-square721 text-base text-zinc-300">
                    Request access to create your OpenClaw with a few easy clicks.
                  </span>
                </div>
                <form className="flex w-full flex-col gap-3" onSubmit={handleSubmit}>
                  {!userEmail && (
                    <input
                      type="email"
                      placeholder="you@example.com"
                      value={email}
                      onChange={(e) => onEmailChange(e.target.value)}
                      disabled={waitlistStatus === 'submitting'}
                      className="font-square721 w-full rounded-md border border-[#1e1830] bg-[#12101a] px-4 py-3 text-base text-zinc-100 placeholder:text-zinc-500 focus:border-[#252030] focus:outline-none focus:ring-1 focus:ring-[#252030] disabled:opacity-60"
                      autoComplete="email"
                      autoFocus
                    />
                  )}
                  {userEmail && (
                    <div className="rounded-md border border-[#1e1830] bg-[#12101a] px-4 py-3 text-sm text-zinc-300">
                      {userEmail}
                    </div>
                  )}
                  <Button
                    type="submit"
                    size="default"
                    variant="secondary"
                    disabled={waitlistStatus === 'submitting'}
                    className="font-square721 border-[#1e1830] bg-[#12101a] text-base text-zinc-100 hover:bg-[#1a1622] disabled:opacity-60"
                  >
                    {waitlistStatus === 'submitting' ? 'Joining…' : 'Join the waitlist'}
                  </Button>
                  {waitlistError && (
                    <p className="font-square721 text-sm text-red-400">{waitlistError}</p>
                  )}
                </form>
              </>
            )}
          </CardContent>
        </Card>
      </div>
    </>
  );
}
