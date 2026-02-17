<?php

namespace App\Services;

use App\Models\Autopilot;
use App\Models\AutopilotPolicy;
use App\Models\AutopilotProfile;
use App\Models\User;
use Illuminate\Database\Eloquent\Collection;
use Illuminate\Database\Eloquent\ModelNotFoundException;
use Illuminate\Support\Facades\DB;

class AutopilotService
{
    public function listForUser(User $owner, int $limit = 100): Collection
    {
        $safeLimit = max(1, min(500, $limit));

        return Autopilot::query()
            ->where('owner_user_id', $owner->id)
            ->orderByDesc('updated_at')
            ->limit($safeLimit)
            ->get();
    }

    public function createForUser(User $owner, array $payload): Autopilot
    {
        return DB::transaction(function () use ($owner, $payload): Autopilot {
            $displayName = $this->normalizeDisplayName((string) ($payload['displayName'] ?? $payload['display_name'] ?? ''));
            $handleSeed = (string) ($payload['handle'] ?? $displayName);
            $handle = $this->generateUniqueHandle($handleSeed);

            $autopilot = Autopilot::query()->create([
                'owner_user_id' => $owner->id,
                'handle' => $handle,
                'display_name' => $displayName,
                'avatar' => $this->nullableString($payload['avatar'] ?? null),
                'tagline' => $this->nullableString($payload['tagline'] ?? null),
                'status' => (string) ($payload['status'] ?? 'active'),
                'visibility' => (string) ($payload['visibility'] ?? 'private'),
                'config_version' => 1,
            ]);

            AutopilotProfile::query()->create([
                'autopilot_id' => $autopilot->id,
                'owner_display_name' => (string) ($owner->name ?: $displayName),
                'onboarding_answers' => [],
                'schema_version' => 1,
            ]);

            AutopilotPolicy::query()->create([
                'autopilot_id' => $autopilot->id,
                'tool_allowlist' => [],
                'tool_denylist' => [],
                'l402_require_approval' => true,
                'l402_allowed_hosts' => [],
            ]);

            return $autopilot->fresh(['profile', 'policy']) ?? $autopilot;
        });
    }

    public function resolveOwned(User $owner, string $reference): Autopilot
    {
        $value = trim($reference);
        $handleCandidate = strtolower($value);

        $autopilot = Autopilot::query()
            ->where('owner_user_id', $owner->id)
            ->where(function ($query) use ($value, $handleCandidate): void {
                $query->where('id', $value)
                    ->orWhere('handle', $handleCandidate);
            })
            ->first();

        if (! $autopilot) {
            throw (new ModelNotFoundException)->setModel(Autopilot::class, [$reference]);
        }

        return $autopilot;
    }

    public function updateOwned(User $owner, string $reference, array $payload): Autopilot
    {
        return DB::transaction(function () use ($owner, $reference, $payload): Autopilot {
            $autopilot = $this->resolveOwned($owner, $reference);

            $updates = [];

            if (array_key_exists('displayName', $payload)) {
                $updates['display_name'] = $this->normalizeDisplayName((string) ($payload['displayName'] ?? ''));
            }
            if (array_key_exists('status', $payload)) {
                $updates['status'] = (string) $payload['status'];
            }
            if (array_key_exists('visibility', $payload)) {
                $updates['visibility'] = (string) $payload['visibility'];
            }
            if (array_key_exists('tagline', $payload)) {
                $updates['tagline'] = $this->nullableString($payload['tagline'] ?? null);
            }
            if (array_key_exists('avatar', $payload)) {
                $updates['avatar'] = $this->nullableString($payload['avatar'] ?? null);
            }

            if ($updates !== []) {
                $autopilot->fill($updates);
                $autopilot->save();
            }

            $shouldIncrementConfig = false;

            if (isset($payload['profile']) && is_array($payload['profile'])) {
                $this->upsertProfile($autopilot, $payload['profile']);
                $shouldIncrementConfig = true;
            }

            if (isset($payload['policy']) && is_array($payload['policy'])) {
                $this->upsertPolicy($autopilot, $payload['policy']);
                $shouldIncrementConfig = true;
            }

            if ($shouldIncrementConfig) {
                $autopilot->config_version = (int) $autopilot->config_version + 1;
                $autopilot->save();
            }

            return $autopilot->fresh(['profile', 'policy']) ?? $autopilot;
        });
    }

    /**
     * @param  array<string, mixed>  $profilePayload
     */
    private function upsertProfile(Autopilot $autopilot, array $profilePayload): void
    {
        $profile = AutopilotProfile::query()->firstOrNew([
            'autopilot_id' => $autopilot->id,
        ]);

        $profile->owner_display_name = (string) ($profilePayload['ownerDisplayName'] ?? $profile->owner_display_name ?? $autopilot->display_name);
        $profile->persona_summary = $this->nullableString($profilePayload['personaSummary'] ?? $profile->persona_summary ?? null);
        $profile->autopilot_voice = $this->nullableString($profilePayload['autopilotVoice'] ?? $profile->autopilot_voice ?? null);
        $profile->principles = $this->normalizeArrayOrNull($profilePayload['principles'] ?? $profile->principles ?? null);
        $profile->preferences = $this->normalizeArrayOrNull($profilePayload['preferences'] ?? $profile->preferences ?? null);
        $profile->onboarding_answers = $this->normalizeArray($profilePayload['onboardingAnswers'] ?? $profile->onboarding_answers ?? []);
        $profile->schema_version = (int) ($profilePayload['schemaVersion'] ?? $profile->schema_version ?? 1);

        $profile->save();
    }

    /**
     * @param  array<string, mixed>  $policyPayload
     */
    private function upsertPolicy(Autopilot $autopilot, array $policyPayload): void
    {
        $policy = AutopilotPolicy::query()->firstOrNew([
            'autopilot_id' => $autopilot->id,
        ]);

        $policy->model_provider = $this->nullableString($policyPayload['modelProvider'] ?? $policy->model_provider ?? null);
        $policy->model = $this->nullableString($policyPayload['model'] ?? $policy->model ?? null);
        $policy->tool_allowlist = $this->normalizeArray($policyPayload['toolAllowlist'] ?? $policy->tool_allowlist ?? []);
        $policy->tool_denylist = $this->normalizeArray($policyPayload['toolDenylist'] ?? $policy->tool_denylist ?? []);
        $policy->l402_require_approval = (bool) ($policyPayload['l402RequireApproval'] ?? $policy->l402_require_approval ?? true);
        $policy->l402_max_spend_msats_per_call = $this->nullableInt($policyPayload['l402MaxSpendMsatsPerCall'] ?? $policy->l402_max_spend_msats_per_call ?? null);
        $policy->l402_max_spend_msats_per_day = $this->nullableInt($policyPayload['l402MaxSpendMsatsPerDay'] ?? $policy->l402_max_spend_msats_per_day ?? null);
        $policy->l402_allowed_hosts = $this->normalizeArray($policyPayload['l402AllowedHosts'] ?? $policy->l402_allowed_hosts ?? []);
        $policy->data_policy = $this->normalizeArrayOrNull($policyPayload['dataPolicy'] ?? $policy->data_policy ?? null);

        $policy->save();
    }

    private function normalizeDisplayName(string $value): string
    {
        $candidate = trim($value);
        if ($candidate === '') {
            return 'Autopilot';
        }

        return mb_substr($candidate, 0, 120);
    }

    private function generateUniqueHandle(string $seed, ?string $ignoreAutopilotId = null): string
    {
        $base = User::normalizeHandleBase($seed);
        if ($base === '') {
            $base = 'autopilot';
        }

        $candidate = $base;
        $suffix = 1;

        while ($this->handleExists($candidate, $ignoreAutopilotId)) {
            $suffixText = '-'.$suffix;
            $trimmed = substr($base, 0, max(1, 64 - strlen($suffixText)));
            $candidate = $trimmed.$suffixText;
            $suffix++;
        }

        return $candidate;
    }

    private function handleExists(string $handle, ?string $ignoreAutopilotId = null): bool
    {
        // Handles are globally unique even across soft-deleted rows due the DB
        // unique index, so include trashed records in collision checks.
        $query = Autopilot::query()->withTrashed()->where('handle', $handle);

        if (is_string($ignoreAutopilotId) && $ignoreAutopilotId !== '') {
            $query->where('id', '!=', $ignoreAutopilotId);
        }

        return $query->exists();
    }

    private function nullableString(mixed $value): ?string
    {
        if (! is_string($value)) {
            return null;
        }

        $trimmed = trim($value);
        if ($trimmed === '') {
            return null;
        }

        return $trimmed;
    }

    private function nullableInt(mixed $value): ?int
    {
        if ($value === null || $value === '') {
            return null;
        }

        return (int) $value;
    }

    /**
     * @return array<int|string, mixed>
     */
    private function normalizeArray(mixed $value): array
    {
        if (! is_array($value)) {
            return [];
        }

        return $value;
    }

    /**
     * @return array<int|string, mixed>|null
     */
    private function normalizeArrayOrNull(mixed $value): ?array
    {
        if ($value === null) {
            return null;
        }

        if (! is_array($value)) {
            return null;
        }

        return $value;
    }
}
