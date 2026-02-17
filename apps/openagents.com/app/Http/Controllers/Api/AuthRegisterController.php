<?php

namespace App\Http\Controllers\Api;

use App\Http\Controllers\Controller;
use App\Models\User;
use App\OpenApi\Responses\DataObjectResponse;
use App\OpenApi\Responses\NotFoundResponse;
use App\OpenApi\Responses\ValidationErrorResponse;
use App\Services\AutopilotService;
use Illuminate\Http\JsonResponse;
use Illuminate\Http\Request;
use Illuminate\Validation\ValidationException;
use Vyuldashev\LaravelOpenApi\Attributes as OpenApi;

#[OpenApi\PathItem]
class AuthRegisterController extends Controller
{
    /**
     * Bootstrap an API user + bearer token (staging/automation).
     *
     * This bypasses WorkOS email code auth and is intended for staging
     * automation. Controlled by `OA_API_SIGNUP_ENABLED`.
     */
    #[OpenApi\Operation(tags: ['Auth'])]
    #[OpenApi\Response(factory: DataObjectResponse::class, statusCode: 201)]
    #[OpenApi\Response(factory: DataObjectResponse::class, statusCode: 200)]
    #[OpenApi\Response(factory: NotFoundResponse::class, statusCode: 404)]
    #[OpenApi\Response(factory: ValidationErrorResponse::class, statusCode: 422)]
    public function store(Request $request, AutopilotService $autopilotService): JsonResponse
    {
        if (! (bool) config('auth.api_signup.enabled', false)) {
            abort(404);
        }

        $validated = $request->validate([
            'email' => ['required', 'email:rfc,dns', 'max:255'],
            'name' => ['nullable', 'string', 'max:120'],
            'tokenName' => ['nullable', 'string', 'max:120'],
            'tokenAbilities' => ['nullable', 'array'],
            'tokenAbilities.*' => ['string', 'max:120'],
            'createAutopilot' => ['nullable', 'boolean'],
            'autopilotDisplayName' => ['nullable', 'string', 'max:120'],
        ]);

        $email = strtolower(trim((string) $validated['email']));
        $this->assertDomainAllowed($email);

        $name = isset($validated['name']) && is_string($validated['name']) && trim($validated['name']) !== ''
            ? trim((string) $validated['name'])
            : $this->defaultNameFromEmail($email);

        $user = User::query()->where('email', $email)->first();
        $created = false;

        if (! $user) {
            $created = true;
            $user = User::query()->create([
                'name' => $name,
                'email' => $email,
                'email_verified_at' => now(),
                'workos_id' => $this->workosIdForEmail($email),
                'avatar' => $this->avatarForEmail($email),
            ]);
        } else {
            if (trim((string) $user->name) === '' || $user->name === $user->email) {
                $user->name = $name;
            }

            if (! $user->email_verified_at) {
                $user->email_verified_at = now();
            }

            if (! is_string($user->workos_id) || trim($user->workos_id) === '') {
                $user->workos_id = $this->workosIdForEmail($email);
            }

            if (! is_string($user->avatar) || trim($user->avatar) === '') {
                $user->avatar = $this->avatarForEmail($email);
            }

            $user->save();
        }

        $tokenName = isset($validated['tokenName']) && is_string($validated['tokenName']) && trim($validated['tokenName']) !== ''
            ? trim((string) $validated['tokenName'])
            : (string) config('auth.api_signup.default_token_name', 'api-bootstrap');

        $tokenAbilities = ['*'];
        if (isset($validated['tokenAbilities']) && is_array($validated['tokenAbilities']) && $validated['tokenAbilities'] !== []) {
            $tokenAbilities = array_values(array_map(
                static fn (string $ability): string => trim($ability),
                array_filter($validated['tokenAbilities'], static fn ($ability): bool => is_string($ability) && trim($ability) !== ''),
            ));

            if ($tokenAbilities === []) {
                $tokenAbilities = ['*'];
            }
        }

        $plainToken = $user->createToken($tokenName, $tokenAbilities)->plainTextToken;

        $autopilotPayload = null;
        $createAutopilot = (bool) ($validated['createAutopilot'] ?? false);

        if ($createAutopilot) {
            $autopilot = $autopilotService->createForUser($user, [
                'displayName' => isset($validated['autopilotDisplayName']) && is_string($validated['autopilotDisplayName'])
                    ? (string) $validated['autopilotDisplayName']
                    : 'Autopilot',
            ]);

            $autopilotPayload = [
                'id' => (string) $autopilot->id,
                'handle' => (string) $autopilot->handle,
                'displayName' => (string) $autopilot->display_name,
                'status' => (string) $autopilot->status,
                'visibility' => (string) $autopilot->visibility,
            ];
        }

        return response()->json([
            'data' => [
                'created' => $created,
                'tokenType' => 'Bearer',
                'token' => $plainToken,
                'tokenName' => $tokenName,
                'tokenAbilities' => $tokenAbilities,
                'user' => [
                    'id' => (int) $user->id,
                    'name' => (string) $user->name,
                    'email' => (string) $user->email,
                    'handle' => (string) $user->handle,
                ],
                'autopilot' => $autopilotPayload,
            ],
        ], $created ? 201 : 200);
    }

    private function assertDomainAllowed(string $email): void
    {
        $domains = config('auth.api_signup.allowed_domains', []);
        if (! is_array($domains) || $domains === []) {
            return;
        }

        $domain = strtolower(trim((string) strstr($email, '@')));
        $domain = ltrim($domain, '@');

        if ($domain === '' || ! in_array($domain, $domains, true)) {
            throw ValidationException::withMessages([
                'email' => 'Email domain is not allowed for API signup in this environment.',
            ]);
        }
    }

    private function defaultNameFromEmail(string $email): string
    {
        $local = (string) explode('@', $email, 2)[0];
        $normalized = trim(str_replace(['.', '-', '_'], ' ', $local));
        $title = ucwords($normalized);

        return $title !== '' ? mb_substr($title, 0, 120) : 'API User';
    }

    private function workosIdForEmail(string $email): string
    {
        return 'api_bootstrap_'.substr(hash('sha256', strtolower(trim($email))), 0, 32);
    }

    private function avatarForEmail(string $email): string
    {
        $hash = md5(strtolower(trim($email)));

        return 'https://www.gravatar.com/avatar/'.$hash.'?d=identicon';
    }
}
