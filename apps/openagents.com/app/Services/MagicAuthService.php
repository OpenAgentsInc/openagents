<?php

namespace App\Services;

use App\Models\User;
use App\Lightning\Spark\UserSparkWalletService;
use Illuminate\Auth\Events\Registered;
use Illuminate\Database\QueryException;
use Illuminate\Support\Carbon;
use Illuminate\Validation\ValidationException;
use Laravel\WorkOS\WorkOS;
use WorkOS\Client;
use WorkOS\Exception\WorkOSException;
use WorkOS\Resource\AuthenticationResponse;
use WorkOS\UserManagement;
use Throwable;

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
        $this->provisionWalletIfConfigured($user);

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

        $email = strtolower(trim($email));
        $name = trim(implode(' ', array_filter([$firstName, $lastName])));

        if ($name === '') {
            $name = $email;
        }

        /** @var User|null $existingByWorkos */
        $existingByWorkos = User::query()->where('workos_id', $workosId)->first();
        /** @var User|null $existingByEmail */
        $existingByEmail = User::query()->where('email', $email)->first();

        // If both match different rows, keep the email-owned account as canonical login identity.
        // This prevents duplicate-key failures when old import/placeholder rows hold a WorkOS id.
        $existingUser = null;
        if ($existingByEmail instanceof User && $existingByWorkos instanceof User && $existingByEmail->id !== $existingByWorkos->id) {
            $existingUser = $existingByEmail;
        } elseif ($existingByWorkos instanceof User) {
            $existingUser = $existingByWorkos;
        } elseif ($existingByEmail instanceof User) {
            $existingUser = $existingByEmail;
        }

        if (! $existingUser instanceof User) {
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

        $safeWorkosId = $this->resolveSafeWorkosId($existingUser, $workosId);

        $existingUser->fill([
            'name' => $name,
            'email' => $email,
            'workos_id' => $safeWorkosId,
            'avatar' => $avatar,
        ]);

        if (! $existingUser->email_verified_at) {
            $existingUser->email_verified_at = now();
        }

        try {
            $existingUser->save();
        } catch (QueryException $exception) {
            report($exception);

            /** @var User|null $fallback */
            $fallback = User::query()
                ->where('email', $email)
                ->orWhere('workos_id', $workosId)
                ->first();

            if ($fallback instanceof User) {
                return $fallback;
            }

            throw ValidationException::withMessages([
                'code' => 'Unable to complete sign-in because this account is already linked. Please try again.',
            ]);
        }

        return $existingUser;
    }

    private function resolveSafeWorkosId(User $existingUser, string $candidateWorkosId): string
    {
        $candidateWorkosId = trim($candidateWorkosId);
        if ($candidateWorkosId === '') {
            return (string) $existingUser->workos_id;
        }

        if ((string) $existingUser->workos_id === $candidateWorkosId) {
            return $candidateWorkosId;
        }

        /** @var User|null $owner */
        $owner = User::query()->where('workos_id', $candidateWorkosId)->first();

        if ($owner instanceof User && (int) $owner->id !== (int) $existingUser->id) {
            return (string) $existingUser->workos_id;
        }

        return $candidateWorkosId;
    }

    private function provisionWalletIfConfigured(User $user): void
    {
        if (! (bool) config('lightning.agent_wallets.auto_provision_on_auth', true)) {
            return;
        }

        $sparkBaseUrl = trim((string) config('lightning.spark_executor.base_url', ''));
        if ($sparkBaseUrl === '') {
            return;
        }

        try {
            resolve(UserSparkWalletService::class)->ensureWalletForUser((int) $user->getAuthIdentifier());
        } catch (Throwable $exception) {
            report($exception);
        }
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
