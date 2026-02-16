<?php

namespace App\Http\Controllers\Auth;

use App\Http\Controllers\Controller;
use App\Http\Requests\Auth\SendEmailCodeRequest;
use App\Http\Requests\Auth\VerifyEmailCodeRequest;
use App\Models\User;
use App\Services\PostHogService;
use Illuminate\Auth\Events\Registered;
use Illuminate\Http\RedirectResponse;
use Illuminate\Http\Request;
use Illuminate\Support\Facades\Auth;
use Illuminate\Validation\ValidationException;
use Inertia\Inertia;
use Inertia\Response;
use Laravel\WorkOS\WorkOS;
use WorkOS\Exception\WorkOSException;
use WorkOS\UserManagement;

class EmailCodeAuthController extends Controller
{
    public function show(Request $request): Response|RedirectResponse
    {
        if ($request->user()) {
            return redirect()->route('chat');
        }

        /** @var array{email?: string}|null $pending */
        $pending = $request->session()->get('auth.magic_auth');

        return Inertia::render('auth/login', [
            'status' => $request->session()->get('status'),
            'pendingEmail' => is_array($pending) ? ($pending['email'] ?? null) : null,
        ]);
    }

    public function sendCode(SendEmailCodeRequest $request): RedirectResponse
    {
        WorkOS::configure();

        $email = $request->validated('email');

        try {
            $magicAuth = (new UserManagement)->createMagicAuth($email);
        } catch (WorkOSException $exception) {
            report($exception);

            throw ValidationException::withMessages([
                'email' => 'Unable to send a sign-in code right now. Please try again.',
            ]);
        }

        $userId = $this->resolveString($magicAuth, ['userId', 'user_id']);

        if (! is_string($userId) || trim($userId) === '') {
            throw ValidationException::withMessages([
                'email' => 'Sign-in provider response was invalid. Please try again.',
            ]);
        }

        $request->session()->put('auth.magic_auth', [
            'email' => $email,
            'user_id' => $userId,
            'sent_at' => now()->toIso8601String(),
        ]);

        // PostHog: Track login code sent
        $posthog = resolve(PostHogService::class);
        $posthog->capture($email, 'login code sent', [
            'method' => 'magic_auth',
        ]);

        return redirect()->route('login')
            ->with('status', 'code-sent');
    }

    public function verifyCode(VerifyEmailCodeRequest $request): RedirectResponse
    {
        WorkOS::configure();

        /** @var array{email?: string, user_id?: string}|null $pending */
        $pending = $request->session()->get('auth.magic_auth');

        $userId = is_array($pending) ? ($pending['user_id'] ?? null) : null;

        if (! is_string($userId) || trim($userId) === '') {
            throw ValidationException::withMessages([
                'code' => 'Your sign-in code expired. Request a new code.',
            ]);
        }

        try {
            $authResponse = (new UserManagement)->authenticateWithMagicAuth(
                config('services.workos.client_id'),
                $request->validated('code'),
                $userId,
                $request->ip(),
                (string) $request->userAgent(),
            );
        } catch (WorkOSException $exception) {
            report($exception);

            throw ValidationException::withMessages([
                'code' => 'That code is invalid or expired. Request a new one.',
            ]);
        }

        $workosUser = $this->resolveObject($authResponse, ['user']);

        if (! is_object($workosUser)) {
            throw ValidationException::withMessages([
                'code' => 'Sign-in provider response was invalid. Please try again.',
            ]);
        }

        $accessToken = $this->resolveString($authResponse, ['accessToken', 'access_token']);
        $refreshToken = $this->resolveString($authResponse, ['refreshToken', 'refresh_token']);

        if (! is_string($accessToken) || trim($accessToken) === '' || ! is_string($refreshToken) || trim($refreshToken) === '') {
            throw ValidationException::withMessages([
                'code' => 'Sign-in provider response was incomplete. Please try again.',
            ]);
        }

        $posthog = resolve(PostHogService::class);
        $isNewUser = false;
        $user = $this->synchronizeUser($workosUser, $isNewUser);

        Auth::guard('web')->login($user);

        $request->session()->put('workos_access_token', $accessToken);
        $request->session()->put('workos_refresh_token', $refreshToken);
        $request->session()->forget('auth.magic_auth');
        $request->session()->regenerate();

        // PostHog: Identify user and track login/signup
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

        return redirect()->intended(route('chat'));
    }

    private function synchronizeUser(object $workosUser, bool &$isNewUser = false): User
    {
        $workosId = $this->resolveString($workosUser, ['id']);
        $email = $this->resolveString($workosUser, ['email']);
        $firstName = $this->resolveString($workosUser, ['firstName', 'first_name']);
        $lastName = $this->resolveString($workosUser, ['lastName', 'last_name']);
        $avatar = $this->resolveString($workosUser, ['profilePictureUrl', 'profile_picture_url']) ?? '';

        if (! is_string($workosId) || trim($workosId) === '' || ! is_string($email) || trim($email) === '') {
            throw ValidationException::withMessages([
                'code' => 'Sign-in provider user payload was invalid. Please try again.',
            ]);
        }

        $name = trim(implode(' ', array_filter([$firstName, $lastName])));

        if ($name === '') {
            $name = $email;
        }

        /** @var User|null $existingUser */
        $existingUser = User::query()
            ->where('workos_id', $workosId)
            ->orWhere('email', $email)
            ->first();

        if (! $existingUser) {
            $isNewUser = true;

            /** @var User $newUser */
            $newUser = User::query()->create([
                'name' => $name,
                'email' => $email,
                'email_verified_at' => now(),
                'workos_id' => $workosId,
                'avatar' => $avatar,
            ]);

            event(new Registered($newUser));

            return $newUser;
        }

        $isNewUser = false;

        $existingUser->fill([
            'name' => $name,
            'email' => $email,
            'workos_id' => $workosId,
            'avatar' => $avatar,
        ]);

        if (! $existingUser->email_verified_at) {
            $existingUser->email_verified_at = now();
        }

        $existingUser->save();

        return $existingUser;
    }

    private function resolveString(object $source, array $keys): ?string
    {
        $value = $this->resolveValue($source, $keys);

        return is_string($value) ? $value : null;
    }

    private function resolveObject(object $source, array $keys): ?object
    {
        $value = $this->resolveValue($source, $keys);

        return is_object($value) ? $value : null;
    }

    private function resolveValue(object $source, array $keys): mixed
    {
        foreach ($keys as $key) {
            try {
                if (! isset($source->{$key})) {
                    continue;
                }

                return $source->{$key};
            } catch (\Throwable) {
                continue;
            }
        }

        return null;
    }
}
