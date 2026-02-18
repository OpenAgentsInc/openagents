<?php

namespace App\Services;

use App\Models\User;
use Illuminate\Auth\Events\Registered;
use Illuminate\Support\Carbon;
use Illuminate\Validation\ValidationException;
use Laravel\WorkOS\WorkOS;
use WorkOS\Client;
use WorkOS\Exception\WorkOSException;
use WorkOS\Resource\AuthenticationResponse;
use WorkOS\UserManagement;

class MagicAuthService
{
    /**
     * @return array{email: string, user_id: string, sent_at: string}
     */
    public function startMagicCode(string $email): array
    {
        WorkOS::configure();

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

        return [
            'email' => $email,
            'user_id' => $userId,
            'sent_at' => Carbon::now()->toIso8601String(),
        ];
    }

    /**
     * @return array{
     *   user: User,
     *   is_new_user: bool,
     *   access_token: string,
     *   refresh_token: string
     * }
     */
    public function verifyMagicCode(string $code, string $pendingUserId, string $ipAddress, string $userAgent, ?string $pendingEmail = null): array
    {
        WorkOS::configure();

        $authResponse = null;

        // Prefer email + code (matches Node SDK and working legacy app); fall back to user_id.
        if ($pendingEmail !== null && $pendingEmail !== '') {
            try {
                $authResponse = $this->authenticateWithMagicAuthUsingEmail(
                    $pendingEmail,
                    $code,
                    $ipAddress,
                    $userAgent,
                );
            } catch (WorkOSException $e) {
                report($e);
                // Fall through to try user_id in case API accepts it.
            }
        }

        if ($authResponse === null) {
            try {
                $authResponse = (new UserManagement)->authenticateWithMagicAuth(
                    config('services.workos.client_id'),
                    $code,
                    $pendingUserId,
                    $ipAddress,
                    $userAgent,
                );
            } catch (WorkOSException $exception) {
                report($exception);

                throw ValidationException::withMessages([
                    'code' => 'That code is invalid or expired. Request a new one.',
                ]);
            }
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

        $isNewUser = false;
        $user = $this->synchronizeUser($workosUser, $isNewUser);

        return [
            'user' => $user,
            'is_new_user' => $isNewUser,
            'access_token' => $accessToken,
            'refresh_token' => $refreshToken,
        ];
    }

    /**
     * Authenticate with magic auth using email + code (matches Node SDK / REST API).
     *
     * @throws WorkOSException
     */
    private function authenticateWithMagicAuthUsingEmail(string $email, string $code, string $ipAddress, string $userAgent): AuthenticationResponse
    {
        $path = 'user_management/authenticate';
        $params = array_filter([
            'client_id' => config('services.workos.client_id'),
            'client_secret' => \WorkOS\WorkOS::getApiKey(),
            'grant_type' => 'urn:workos:oauth:grant-type:magic-auth:code',
            'email' => $email,
            'code' => $code,
            'ip_address' => $ipAddress !== '' ? $ipAddress : null,
            'user_agent' => $userAgent !== '' ? $userAgent : null,
        ], fn ($v) => $v !== null && $v !== '');

        $response = Client::request(Client::METHOD_POST, $path, null, $params, true);

        return AuthenticationResponse::constructFromResponse($response);
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
