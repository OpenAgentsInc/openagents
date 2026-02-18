import { Head, useForm, usePage } from '@inertiajs/react';
import { useEffect } from 'react';
import type { FormEvent } from 'react';
import InputError from '@/components/input-error';
import { Button } from '@/components/ui/button';
import { Input } from '@/components/ui/input';
import { Label } from '@/components/ui/label';
import { usePostHogEvent } from '@/hooks/use-posthog-event';

type LoginProps = {
    pendingEmail?: string | null;
    status?: string | null;
};

export default function Login() {
    const { pendingEmail, status } = usePage<LoginProps>().props;
    const capture = usePostHogEvent('login');

    const emailForm = useForm({
        email: pendingEmail ?? '',
    });

    const verifyForm = useForm({
        code: '',
    });

    const hasPendingCode = typeof pendingEmail === 'string' && pendingEmail.length > 0;

    useEffect(() => {
        capture('login.page_opened', {
            hasPendingCode,
            hasStatusCodeSent: status === 'code-sent',
        });
    }, [capture, hasPendingCode, status]);

    const submitEmail = (event: FormEvent<HTMLFormElement>) => {
        event.preventDefault();
        capture('login.code_send_submitted', {
            emailLength: emailForm.data.email.trim().length,
        });

        emailForm.post('/login/email', {
            preserveScroll: true,
        });
    };

    const submitCode = (event: FormEvent<HTMLFormElement>) => {
        event.preventDefault();
        capture('login.code_verify_submitted', {
            codeLength: verifyForm.data.code.trim().length,
        });

        verifyForm.post('/login/verify', {
            preserveScroll: true,
        });
    };

    return (
        <>
            <Head title="Log in" />

            <div className="relative min-h-screen overflow-hidden bg-black text-white">
                <div className="pointer-events-none absolute inset-0 [background-image:radial-gradient(circle_at_center,rgba(255,255,255,0.075)_1px,transparent_1px)] [background-size:36px_36px]" />

                <div className="absolute inset-0 z-10 flex items-center justify-center overflow-auto px-6 py-12">
                    <div className="w-full max-w-md rounded-xl border border-white/20 bg-black/40 p-6 shadow-2xl backdrop-blur">
                        <div className="mb-6">
                            <h1 className="text-2xl font-semibold tracking-tight">Log in with email</h1>
                            <p className="mt-1 text-sm text-white/75">
                                {hasPendingCode
                                    ? `Enter the one-time code sent to ${pendingEmail}.`
                                    : 'Enter your email to receive a one-time sign-in code.'}
                            </p>
                        </div>

                        {status === 'code-sent' ? (
                            <div className="mb-4 rounded-md border border-emerald-300/40 bg-emerald-500/10 px-3 py-2 text-sm text-emerald-200">
                                Verification code sent. Check your inbox and enter it below.
                            </div>
                        ) : null}

                        {hasPendingCode ? (
                            <form onSubmit={submitCode} className="space-y-3">
                                <div className="space-y-2">
                                    <Label htmlFor="code">Verification code</Label>
                                    <Input
                                        id="code"
                                        type="text"
                                        autoComplete="one-time-code"
                                        autoFocus
                                        value={verifyForm.data.code}
                                        onChange={(event) => verifyForm.setData('code', event.target.value)}
                                        placeholder="Enter the code from your email"
                                        className="focus-visible:border-white focus-visible:ring-white/50"
                                    />
                                    <InputError message={verifyForm.errors.code} />
                                </div>

                                <Button type="submit" className="w-full" disabled={verifyForm.processing}>
                                    {verifyForm.processing ? 'Verifying...' : 'Verify and continue'}
                                </Button>
                            </form>
                        ) : (
                            <form onSubmit={submitEmail} className="space-y-3">
                                <div className="space-y-2">
                                    <Label htmlFor="email">Email</Label>
                                    <Input
                                        id="email"
                                        type="email"
                                        autoComplete="email"
                                        autoFocus
                                        value={emailForm.data.email}
                                        onChange={(event) => emailForm.setData('email', event.target.value)}
                                        placeholder="you@openagents.com"
                                        className="focus-visible:border-white focus-visible:ring-white/50"
                                    />
                                    <InputError message={emailForm.errors.email} />
                                </div>

                                <Button type="submit" className="w-full" disabled={emailForm.processing}>
                                    {emailForm.processing ? 'Sending code...' : 'Send verification code'}
                                </Button>
                            </form>
                        )}
                    </div>
                </div>
            </div>
        </>
    );
}
