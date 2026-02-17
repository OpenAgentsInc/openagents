<?php

namespace App\AI\Tools\Concerns;

use App\Models\L402Paywall;
use App\Models\User;
use Illuminate\Support\Facades\Validator;
use RuntimeException;

trait L402PaywallToolSupport
{
    private function resolveAuthenticatedUser(): ?User
    {
        $user = auth()->user();
        if (! $user instanceof User) {
            return null;
        }

        return $user;
    }

    /**
     * @param  array<string, mixed>  $raw
     * @return array{ok:bool,validated:array<string,mixed>,errors:array<string,mixed>}
     */
    private function validateCreatePayload(array $raw): array
    {
        return $this->validatePayload($raw, [
            'name' => ['required', 'string', 'max:120'],
            'hostRegexp' => ['required', 'string', 'max:255', function (string $attribute, mixed $value, \Closure $fail): void {
                $this->validateRegexBody($attribute, $value, $fail, false);
            }],
            'pathRegexp' => ['required', 'string', 'max:255', function (string $attribute, mixed $value, \Closure $fail): void {
                $this->validateRegexBody($attribute, $value, $fail, true);
            }],
            'priceMsats' => ['required', 'integer', 'min:1', 'max:1000000000000'],
            'upstream' => ['required', 'string', 'url', 'max:2048', 'starts_with:http://,https://'],
            'enabled' => ['sometimes', 'boolean'],
            'metadata' => ['sometimes', 'array'],
        ]);
    }

    /**
     * @param  array<string, mixed>  $raw
     * @return array{ok:bool,validated:array<string,mixed>,errors:array<string,mixed>}
     */
    private function validateUpdatePayload(array $raw): array
    {
        $validated = $this->validatePayload($raw, [
            'paywallId' => ['required', 'string', 'max:36'],
            'name' => ['sometimes', 'required', 'string', 'max:120'],
            'hostRegexp' => ['sometimes', 'required', 'string', 'max:255', function (string $attribute, mixed $value, \Closure $fail): void {
                $this->validateRegexBody($attribute, $value, $fail, false);
            }],
            'pathRegexp' => ['sometimes', 'required', 'string', 'max:255', function (string $attribute, mixed $value, \Closure $fail): void {
                $this->validateRegexBody($attribute, $value, $fail, true);
            }],
            'priceMsats' => ['sometimes', 'required', 'integer', 'min:1', 'max:1000000000000'],
            'upstream' => ['sometimes', 'required', 'string', 'url', 'max:2048', 'starts_with:http://,https://'],
            'enabled' => ['sometimes', 'boolean'],
            'metadata' => ['sometimes', 'array'],
        ]);

        if ($validated['ok'] !== true) {
            return $validated;
        }

        $updateFields = $validated['validated'];
        unset($updateFields['paywallId']);

        if ($updateFields === []) {
            return [
                'ok' => false,
                'validated' => [],
                'errors' => ['payload' => ['At least one mutable paywall field must be provided.']],
            ];
        }

        return $validated;
    }

    /**
     * @param  array<string, mixed>  $raw
     * @return array{ok:bool,validated:array<string,mixed>,errors:array<string,mixed>}
     */
    private function validateDeletePayload(array $raw): array
    {
        return $this->validatePayload($raw, [
            'paywallId' => ['required', 'string', 'max:36'],
        ]);
    }

    /**
     * @param  array<string, mixed>  $raw
     * @param  array<string, mixed>  $rules
     * @return array{ok:bool,validated:array<string,mixed>,errors:array<string,mixed>}
     */
    private function validatePayload(array $raw, array $rules): array
    {
        $validator = Validator::make($raw, $rules);

        if ($validator->fails()) {
            return [
                'ok' => false,
                'validated' => [],
                'errors' => $validator->errors()->toArray(),
            ];
        }

        return [
            'ok' => true,
            'validated' => $validator->validated(),
            'errors' => [],
        ];
    }

    /**
     * @param  \Closure(string): void  $fail
     */
    private function validateRegexBody(string $attribute, mixed $value, \Closure $fail, bool $pathScoped): void
    {
        if (! is_string($value) || trim($value) === '') {
            $fail("The {$attribute} must be a non-empty regex body.");

            return;
        }

        $normalized = trim($value);

        $dangerousCatchAll = ['.*', '^.*', '^.*$', '.*$'];
        if (in_array($normalized, $dangerousCatchAll, true)) {
            $fail("The {$attribute} is too broad.");

            return;
        }

        if ($pathScoped && ! str_starts_with($normalized, '^/')) {
            $fail("The {$attribute} must start with '^/' to scope path matching.");

            return;
        }

        if ($pathScoped && preg_match('/^\^\/\.\*(\$)?$/', $normalized) === 1) {
            $fail("The {$attribute} cannot be a catch-all path expression.");

            return;
        }

        if (! $pathScoped && ! str_contains($normalized, '\\.')) {
            $fail("The {$attribute} must include an explicit host pattern.");

            return;
        }

        $candidate = '/'.str_replace('/', '\\/', $normalized).'/';

        if (@preg_match($candidate, 'openagents') === false) {
            $fail("The {$attribute} must be a valid regex body.");
        }
    }

    /**
     * @param  array<string, mixed>  $payload
     */
    private function encodePayload(array $payload): string
    {
        $json = json_encode($payload, JSON_UNESCAPED_SLASHES | JSON_UNESCAPED_UNICODE);
        if (! is_string($json)) {
            throw new RuntimeException('Failed to encode paywall tool payload.');
        }

        return $json;
    }

    /**
     * @return array<string, mixed>
     */
    private function paywallSummary(L402Paywall $paywall): array
    {
        return [
            'id' => (string) $paywall->id,
            'name' => (string) $paywall->name,
            'hostRegexp' => (string) $paywall->host_regexp,
            'pathRegexp' => (string) $paywall->path_regexp,
            'priceMsats' => (int) $paywall->price_msats,
            'upstream' => (string) $paywall->upstream,
            'enabled' => (bool) $paywall->enabled,
            'deletedAt' => $paywall->deleted_at?->toISOString(),
            'lastReconcileStatus' => $paywall->last_reconcile_status,
            'lastReconciledAt' => $paywall->last_reconciled_at?->toISOString(),
        ];
    }

    private function operationId(string $eventType, mixed $mutationEventId): ?string
    {
        if (! is_numeric($mutationEventId)) {
            return null;
        }

        return $eventType.':'.(int) $mutationEventId;
    }
}
