<?php

namespace App\Http\Controllers\Auth;

use App\Http\Controllers\Controller;
use App\Http\Requests\Auth\SendEmailCodeRequest;
use App\Http\Requests\Auth\VerifyEmailCodeRequest;
use App\Models\User;
use App\Services\MagicAuthService;
use App\Services\PostHogService;
use Illuminate\Http\JsonResponse;
use Illuminate\Http\RedirectResponse;
use Illuminate\Http\Request;
use Illuminate\Support\Facades\Auth;
use Illuminate\Validation\ValidationException;
use Inertia\Inertia;
use Inertia\Response;

class EmailCodeAuthController extends Controller
{
    public function show(Request $request): Response|RedirectResponse
    {
        if ($request->user()) {
            return redirect()->route('home');
        }

        /** @var array{email?: string}|null $pending */
        $pending = $request->session()->get('auth.magic_auth');

        return Inertia::render('auth/login', [
            'status' => $request->session()->get('status'),
            'pendingEmail' => is_array($pending) ? ($pending['email'] ?? null) : null,
        ]);
    }

    public function sendCode(SendEmailCodeRequest $request, MagicAuthService $magicAuthService, PostHogService $posthog): RedirectResponse
    {
        $email = trim((string) $request->validated('email'));
        $pending = $magicAuthService->startMagicCode($email);

        $request->session()->put('auth.magic_auth', $pending);

        $posthog->capture($email, 'login code sent', [
            'method' => 'magic_auth',
        ]);

        return redirect()->route('login')
            ->with('status', 'code-sent');
    }

    public function sendCodeJson(SendEmailCodeRequest $request, MagicAuthService $magicAuthService, PostHogService $posthog): JsonResponse
    {
        $email = trim((string) $request->validated('email'));
        $pending = $magicAuthService->startMagicCode($email);

        $request->session()->put('auth.magic_auth', $pending);

        $posthog->capture($email, 'login code sent', [
            'method' => 'magic_auth',
            'source' => 'chat_onboarding',
        ]);

        return response()->json([
            'ok' => true,
            'status' => 'code-sent',
            'email' => $email,
        ]);
    }

    public function verifyCode(VerifyEmailCodeRequest $request, MagicAuthService $magicAuthService, PostHogService $posthog): RedirectResponse
    {
        $pending = $this->pendingMagicAuthOrFail($request);

        $verified = $magicAuthService->verifyMagicCode(
            code: trim((string) $request->validated('code')),
            pendingUserId: (string) $pending['user_id'],
            ipAddress: (string) $request->ip(),
            userAgent: (string) $request->userAgent(),
        );

        $this->completeSignIn($request, $verified, $posthog);

        return redirect()->intended(route('home'));
    }

    public function verifyCodeJson(VerifyEmailCodeRequest $request, MagicAuthService $magicAuthService, PostHogService $posthog): JsonResponse
    {
        $pending = $this->pendingMagicAuthOrFail($request);

        $verified = $magicAuthService->verifyMagicCode(
            code: trim((string) $request->validated('code')),
            pendingUserId: (string) $pending['user_id'],
            ipAddress: (string) $request->ip(),
            userAgent: (string) $request->userAgent(),
        );

        /** @var User $user */
        $user = $this->completeSignIn($request, $verified, $posthog);

        $response = [
            'ok' => true,
            'userId' => (string) $user->id,
            'status' => 'authenticated',
            'user' => [
                'id' => $user->id,
                'name' => $user->name,
                'email' => $user->email,
            ],
            'redirect' => '/',
        ];

        $clientName = $this->apiClientName($request);
        if ($clientName !== null) {
            $response['tokenType'] = 'Bearer';
            $response['token'] = $this->issueMobileApiToken($user, $clientName);
            $response['tokenName'] = $this->apiTokenName($clientName);
        }

        return response()->json($response);
    }

    /**
     * @return array{email: string, user_id: string}
     */
    private function pendingMagicAuthOrFail(Request $request): array
    {
        /** @var array{email?: string, user_id?: string}|null $pending */
        $pending = $request->session()->get('auth.magic_auth');

        $userId = is_array($pending) ? ($pending['user_id'] ?? null) : null;
        $email = is_array($pending) ? ($pending['email'] ?? null) : null;

        if (! is_string($userId) || trim($userId) === '' || ! is_string($email) || trim($email) === '') {
            throw ValidationException::withMessages([
                'code' => 'Your sign-in code expired. Request a new code.',
            ]);
        }

        return [
            'email' => trim($email),
            'user_id' => trim($userId),
        ];
    }

    /**
     * @param  array{user: User, is_new_user: bool, access_token: string, refresh_token: string}  $verified
     */
    private function completeSignIn(Request $request, array $verified, PostHogService $posthog): User
    {
        /** @var User $user */
        $user = $verified['user'];
        $isNewUser = (bool) ($verified['is_new_user'] ?? false);
        $accessToken = (string) $verified['access_token'];
        $refreshToken = (string) $verified['refresh_token'];

        Auth::guard('web')->login($user);

        $request->session()->put('workos_access_token', $accessToken);
        $request->session()->put('workos_refresh_token', $refreshToken);
        $request->session()->forget('auth.magic_auth');
        $request->session()->regenerate();

        $posthog->identify($user->email, $user->getPostHogProperties());

        if ($isNewUser) {
            $posthog->capture($user->email, 'user signed up', [
                'signup_method' => 'magic_auth',
            ]);
        } else {
            $posthog->capture($user->email, 'user logged in', [
                'login_method' => 'magic_auth',
            ]);
        }

        return $user;
    }

    private function apiClientName(Request $request): ?string
    {
        $client = strtolower(trim((string) $request->header('x-client', '')));
        if ($client === '') {
            return null;
        }

        return in_array($client, ['autopilot-ios', 'openagents-expo', 'autopilot-desktop', 'openagents-desktop'], true)
            ? $client
            : null;
    }

    private function issueMobileApiToken(User $user, string $clientName): string
    {
        $tokenName = $this->apiTokenName($clientName);

        $user->tokens()->where('name', $tokenName)->delete();

        return $user->createToken($tokenName, ['*'])->plainTextToken;
    }

    private function apiTokenName(string $clientName): string
    {
        if (in_array($clientName, ['autopilot-desktop', 'openagents-desktop'], true)) {
            return 'desktop:'.$clientName;
        }

        return 'mobile:'.$clientName;
    }
}
