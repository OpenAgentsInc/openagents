<?php

namespace App\Lightning\L402;

use App\AI\Runtime\AutopilotExecutionContext;
use App\Models\AutopilotPolicy;

final class L402PolicyEnforcer
{
    public function __construct(private readonly AutopilotExecutionContext $executionContext) {}

    /**
     * @return array{policySource:string,autopilotId:?string,effectiveRequireApproval:bool,effectiveMaxSpendMsats:int,effectiveMaxSpendSats:int,allowedHosts:?array<int,string>,denyCode:?string,denyReason:?array<string,mixed>}
     */
    public function evaluate(string $url, int $requestedMaxSpendMsats, bool $requestedRequireApproval, ?string $autopilotId = null): array
    {
        $resolvedAutopilotId = $this->normalizeAutopilotId($autopilotId) ?? $this->executionContext->autopilotId();
        $policy = $this->resolveAutopilotPolicy($resolvedAutopilotId);

        if (! $policy) {
            return [
                'policySource' => 'config',
                'autopilotId' => null,
                'effectiveRequireApproval' => $requestedRequireApproval,
                'effectiveMaxSpendMsats' => $requestedMaxSpendMsats,
                'effectiveMaxSpendSats' => $this->safeSatsFromMsats($requestedMaxSpendMsats),
                'allowedHosts' => null,
                'denyCode' => null,
                'denyReason' => null,
            ];
        }

        $host = $this->hostFromUrl($url);
        if (! is_string($host)) {
            return [
                'policySource' => 'autopilot',
                'autopilotId' => $policy->autopilot_id,
                'effectiveRequireApproval' => (bool) $policy->l402_require_approval,
                'effectiveMaxSpendMsats' => $requestedMaxSpendMsats,
                'effectiveMaxSpendSats' => $this->safeSatsFromMsats($requestedMaxSpendMsats),
                'allowedHosts' => $this->normalizeHosts($policy->l402_allowed_hosts),
                'denyCode' => 'url_invalid',
                'denyReason' => [
                    'url' => $url,
                ],
            ];
        }

        $allowedHosts = $this->normalizeHosts($policy->l402_allowed_hosts);
        if (! in_array($host, $allowedHosts, true)) {
            return [
                'policySource' => 'autopilot',
                'autopilotId' => $policy->autopilot_id,
                'effectiveRequireApproval' => (bool) $policy->l402_require_approval,
                'effectiveMaxSpendMsats' => $requestedMaxSpendMsats,
                'effectiveMaxSpendSats' => $this->safeSatsFromMsats($requestedMaxSpendMsats),
                'allowedHosts' => $allowedHosts,
                'denyCode' => 'domain_not_allowed',
                'denyReason' => [
                    'host' => $host,
                    'allowedHosts' => $allowedHosts,
                ],
            ];
        }

        $policyCapMsats = $this->normalizePositiveInt($policy->l402_max_spend_msats_per_call);
        if (is_int($policyCapMsats) && $requestedMaxSpendMsats > $policyCapMsats) {
            return [
                'policySource' => 'autopilot',
                'autopilotId' => $policy->autopilot_id,
                'effectiveRequireApproval' => (bool) $policy->l402_require_approval,
                'effectiveMaxSpendMsats' => $policyCapMsats,
                'effectiveMaxSpendSats' => $this->safeSatsFromMsats($policyCapMsats),
                'allowedHosts' => $allowedHosts,
                'denyCode' => 'max_spend_exceeds_policy_cap',
                'denyReason' => [
                    'requestedMaxSpendMsats' => $requestedMaxSpendMsats,
                    'policyCapMsats' => $policyCapMsats,
                ],
            ];
        }

        return [
            'policySource' => 'autopilot',
            'autopilotId' => $policy->autopilot_id,
            'effectiveRequireApproval' => (bool) $policy->l402_require_approval,
            'effectiveMaxSpendMsats' => $requestedMaxSpendMsats,
            'effectiveMaxSpendSats' => $this->safeSatsFromMsats($requestedMaxSpendMsats),
            'allowedHosts' => $allowedHosts,
            'denyCode' => null,
            'denyReason' => null,
        ];
    }

    private function resolveAutopilotPolicy(?string $autopilotId): ?AutopilotPolicy
    {
        if (! is_string($autopilotId) || trim($autopilotId) === '') {
            return null;
        }

        return AutopilotPolicy::query()->find(trim($autopilotId));
    }

    /**
     * @param  mixed  $value
     */
    private function normalizePositiveInt($value): ?int
    {
        if (! is_numeric($value)) {
            return null;
        }

        $intValue = (int) $value;

        return $intValue > 0 ? $intValue : null;
    }

    /**
     * @param  mixed  $value
     * @return array<int, string>
     */
    private function normalizeHosts($value): array
    {
        if (! is_array($value)) {
            return [];
        }

        $hosts = [];

        foreach ($value as $host) {
            if (! is_string($host)) {
                continue;
            }

            $candidate = strtolower(trim($host));
            if ($candidate === '') {
                continue;
            }

            $hosts[$candidate] = $candidate;
        }

        return array_values($hosts);
    }

    private function normalizeAutopilotId(?string $autopilotId): ?string
    {
        if (! is_string($autopilotId)) {
            return null;
        }

        $trimmed = trim($autopilotId);

        return $trimmed === '' ? null : $trimmed;
    }

    private function hostFromUrl(string $url): ?string
    {
        $host = parse_url($url, PHP_URL_HOST);

        if (! is_string($host) || trim($host) === '') {
            return null;
        }

        return strtolower(trim($host));
    }

    private function safeSatsFromMsats(int $msats): int
    {
        $msats = max(0, $msats);

        if ($msats === 0) {
            return 0;
        }

        $sats = intdiv($msats + 999, 1000);

        return $sats;
    }
}
