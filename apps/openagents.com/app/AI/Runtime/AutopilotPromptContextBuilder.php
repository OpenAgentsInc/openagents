<?php

namespace App\AI\Runtime;

use Illuminate\Support\Facades\DB;
use Illuminate\Support\Str;

class AutopilotPromptContextBuilder
{
    public function __construct(
        private readonly AutopilotExecutionContext $executionContext,
    ) {}

    public function forCurrentAutopilot(): ?string
    {
        return $this->forAutopilot($this->executionContext->autopilotId());
    }

    public function forAutopilot(?string $autopilotId): ?string
    {
        $candidate = is_string($autopilotId) ? trim($autopilotId) : '';
        if ($candidate === '') {
            return null;
        }

        $row = DB::table('autopilots as a')
            ->leftJoin('autopilot_profiles as p', 'p.autopilot_id', '=', 'a.id')
            ->leftJoin('autopilot_policies as pol', 'pol.autopilot_id', '=', 'a.id')
            ->where('a.id', $candidate)
            ->first([
                'a.id',
                'a.handle',
                'a.display_name',
                'a.tagline',
                'a.config_version',
                'p.owner_display_name',
                'p.persona_summary',
                'p.autopilot_voice',
                'p.principles',
                'p.preferences',
                'p.onboarding_answers',
                'pol.tool_allowlist',
                'pol.tool_denylist',
                'pol.l402_require_approval',
                'pol.l402_max_spend_msats_per_call',
                'pol.l402_max_spend_msats_per_day',
                'pol.l402_allowed_hosts',
            ]);

        if (! $row) {
            return null;
        }

        $principles = $this->decodeJsonArray($row->principles ?? null);
        $preferences = $this->decodeJsonObject($row->preferences ?? null);
        $onboarding = $this->decodeJsonObject($row->onboarding_answers ?? null);
        $allowlist = $this->decodeJsonArray($row->tool_allowlist ?? null);
        $denylist = $this->decodeJsonArray($row->tool_denylist ?? null);
        $allowedHosts = $this->decodeJsonArray($row->l402_allowed_hosts ?? null);

        $lines = [];
        $lines[] = 'autopilot_id='.$row->id;
        $lines[] = 'config_version='.(int) ($row->config_version ?? 1);
        $lines[] = 'handle='.(string) ($row->handle ?? '');

        $displayName = $this->nullableString($row->display_name ?? null);
        if ($displayName !== null) {
            $lines[] = 'display_name='.$displayName;
        }

        $ownerDisplayName = $this->nullableString($row->owner_display_name ?? null);
        if ($ownerDisplayName !== null) {
            $lines[] = 'owner_display_name='.$ownerDisplayName;
        }

        $tagline = $this->nullableString($row->tagline ?? null);
        if ($tagline !== null) {
            $lines[] = 'tagline='.$tagline;
        }

        $persona = $this->nullableString($row->persona_summary ?? null);
        if ($persona !== null) {
            $lines[] = 'persona_summary='.$persona;
        }

        $voice = $this->nullableString($row->autopilot_voice ?? null);
        if ($voice !== null) {
            $lines[] = 'autopilot_voice='.$voice;
        }

        if ($principles !== []) {
            $lines[] = 'principles='.implode(' | ', array_map(static fn ($v) => (string) $v, $principles));
        }

        if ($preferences !== []) {
            $userPrefs = is_array($preferences['user'] ?? null) ? $preferences['user'] : [];
            $characterPrefs = is_array($preferences['character'] ?? null) ? $preferences['character'] : [];

            $addressAs = $this->nullableString($userPrefs['addressAs'] ?? null);
            if ($addressAs !== null) {
                $lines[] = 'preferred_address='.$addressAs;
            }

            $timeZone = $this->nullableString($userPrefs['timeZone'] ?? null);
            if ($timeZone !== null) {
                $lines[] = 'time_zone='.$timeZone;
            }

            $boundaries = is_array($characterPrefs['boundaries'] ?? null)
                ? array_values(array_filter($characterPrefs['boundaries'], fn ($v) => is_string($v) && trim((string) $v) !== ''))
                : [];
            if ($boundaries !== []) {
                $lines[] = 'boundaries='.implode(' | ', $boundaries);
            }
        }

        if ($onboarding !== []) {
            $bootstrapState = is_array($onboarding['bootstrapState'] ?? null) ? $onboarding['bootstrapState'] : [];
            if ($bootstrapState !== []) {
                $status = $this->nullableString($bootstrapState['status'] ?? null);
                $stage = $this->nullableString($bootstrapState['stage'] ?? null);

                if ($status !== null || $stage !== null) {
                    $lines[] = 'onboarding='.trim(($status ?? 'unknown').($stage !== null ? ' @ '.$stage : ''));
                }
            }
        }

        if ($allowlist !== []) {
            $lines[] = 'tool_allowlist='.implode(',', array_map(static fn ($v) => (string) $v, $allowlist));
        }

        if ($denylist !== []) {
            $lines[] = 'tool_denylist='.implode(',', array_map(static fn ($v) => (string) $v, $denylist));
        }

        $l402RequireApproval = (bool) ($row->l402_require_approval ?? true);
        $lines[] = 'l402_require_approval='.($l402RequireApproval ? 'true' : 'false');

        if (is_numeric($row->l402_max_spend_msats_per_call ?? null)) {
            $lines[] = 'l402_max_spend_msats_per_call='.(int) $row->l402_max_spend_msats_per_call;
        }

        if (is_numeric($row->l402_max_spend_msats_per_day ?? null)) {
            $lines[] = 'l402_max_spend_msats_per_day='.(int) $row->l402_max_spend_msats_per_day;
        }

        if ($allowedHosts !== []) {
            $lines[] = 'l402_allowed_hosts='.implode(',', array_map(static fn ($v) => (string) $v, $allowedHosts));
        }

        if ($lines === []) {
            return null;
        }

        return (string) Str::of(implode("\n", $lines))->trim()->limit(3200, '');
    }

    /**
     * @return array<int, mixed>
     */
    private function decodeJsonArray(mixed $value): array
    {
        if (is_array($value)) {
            return array_values($value);
        }

        if (! is_string($value) || trim($value) === '') {
            return [];
        }

        $decoded = json_decode($value, true);

        return is_array($decoded) ? array_values($decoded) : [];
    }

    /**
     * @return array<string, mixed>
     */
    private function decodeJsonObject(mixed $value): array
    {
        if (is_array($value)) {
            return $value;
        }

        if (! is_string($value) || trim($value) === '') {
            return [];
        }

        $decoded = json_decode($value, true);

        return is_array($decoded) ? $decoded : [];
    }

    private function nullableString(mixed $value): ?string
    {
        if (! is_string($value)) {
            return null;
        }

        $trimmed = trim($value);

        return $trimmed !== '' ? $trimmed : null;
    }
}
