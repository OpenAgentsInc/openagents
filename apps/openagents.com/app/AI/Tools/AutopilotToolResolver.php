<?php

namespace App\AI\Tools;

use App\AI\Runtime\AutopilotExecutionContext;
use App\Models\AutopilotPolicy;
use Laravel\Ai\Contracts\Tool;

class AutopilotToolResolver
{
    /** @var list<string> */
    private const GUEST_ALLOWED_TOOLS = ['chat_login', 'openagents_api'];

    public function __construct(
        private readonly ToolRegistry $toolRegistry,
        private readonly AutopilotExecutionContext $executionContext,
    ) {}

    /**
     * @return list<Tool>
     */
    public function forCurrentAutopilot(): array
    {
        return $this->forAutopilot($this->executionContext->autopilotId());
    }

    /**
     * @return list<Tool>
     */
    public function forAutopilot(?string $autopilotId): array
    {
        return $this->resolutionForAutopilot($autopilotId)['tools'];
    }

    /**
     * @return array{tools:list<Tool>,audit:array<string,mixed>}
     */
    public function resolutionForAutopilot(?string $autopilotId): array
    {
        $allTools = $this->toolRegistry->all();
        $toolByName = [];

        foreach ($allTools as $tool) {
            $name = strtolower(trim($tool->name()));
            if ($name === '') {
                continue;
            }

            $toolByName[$name] = $tool;
        }

        $allAvailableToolNames = array_keys($toolByName);

        if (! $this->executionContext->authenticatedSession()) {
            return $this->guestOnlyResolution($toolByName, $allAvailableToolNames);
        }

        // Guests authenticate via `chat_login`; hide it for authenticated runs.
        $authenticatedToolByName = $toolByName;
        unset($authenticatedToolByName['chat_login']);

        $availableToolNames = array_keys($authenticatedToolByName);

        $policy = $this->resolvePolicy($autopilotId);
        if (! $policy) {
            return [
                'tools' => array_values($authenticatedToolByName),
                'audit' => [
                    'policyApplied' => false,
                    'authRestricted' => false,
                    'sessionAuthenticated' => true,
                    'autopilotId' => null,
                    'availableTools' => $availableToolNames,
                    'exposedTools' => $availableToolNames,
                    'allowlist' => [],
                    'denylist' => [],
                    'removedByAllowlist' => [],
                    'removedByDenylist' => [],
                    'removedByAuthGate' => [],
                ],
            ];
        }

        $allowlist = $this->normalizeNameList($policy->tool_allowlist);
        $denylist = $this->normalizeNameList($policy->tool_denylist);

        $resolved = self::resolveToolNames($availableToolNames, $allowlist, $denylist);

        $resolvedTools = [];
        foreach ($resolved['exposed'] as $name) {
            if (isset($authenticatedToolByName[$name])) {
                $resolvedTools[] = $authenticatedToolByName[$name];
            }
        }

        return [
            'tools' => $resolvedTools,
            'audit' => [
                'policyApplied' => true,
                'authRestricted' => false,
                'sessionAuthenticated' => true,
                'autopilotId' => $policy->autopilot_id,
                'availableTools' => $availableToolNames,
                'exposedTools' => $resolved['exposed'],
                'allowlist' => $allowlist,
                'denylist' => $denylist,
                'removedByAllowlist' => $resolved['removedByAllowlist'],
                'removedByDenylist' => $resolved['removedByDenylist'],
                'removedByAuthGate' => [],
            ],
        ];
    }

    /**
     * Deterministic allow/deny filtering for tool names.
     *
     * @param  list<string>  $availableToolNames
     * @param  list<string>  $allowlist
     * @param  list<string>  $denylist
     * @return array{exposed:list<string>,removedByAllowlist:list<string>,removedByDenylist:list<string>}
     */
    public static function resolveToolNames(array $availableToolNames, array $allowlist, array $denylist): array
    {
        $available = self::normalizeStaticNameList($availableToolNames);
        $allow = self::normalizeStaticNameList($allowlist);
        $deny = self::normalizeStaticNameList($denylist);

        $allowSet = array_fill_keys($allow, true);
        $denySet = array_fill_keys($deny, true);

        $candidate = [];
        $removedByAllowlist = [];

        foreach ($available as $name) {
            if ($allow !== [] && ! isset($allowSet[$name])) {
                $removedByAllowlist[] = $name;

                continue;
            }

            $candidate[] = $name;
        }

        $exposed = [];
        $removedByDenylist = [];

        foreach ($candidate as $name) {
            if (isset($denySet[$name])) {
                $removedByDenylist[] = $name;

                continue;
            }

            $exposed[] = $name;
        }

        return [
            'exposed' => $exposed,
            'removedByAllowlist' => $removedByAllowlist,
            'removedByDenylist' => $removedByDenylist,
        ];
    }

    private function resolvePolicy(?string $autopilotId): ?AutopilotPolicy
    {
        if (! is_string($autopilotId)) {
            return null;
        }

        $trimmed = trim($autopilotId);
        if ($trimmed === '') {
            return null;
        }

        return AutopilotPolicy::query()->find($trimmed);
    }

    /**
     * @param  mixed  $value
     * @return list<string>
     */
    private function normalizeNameList($value): array
    {
        if (! is_array($value)) {
            return [];
        }

        return self::normalizeStaticNameList($value);
    }

    /**
     * @param  mixed  $value
     * @return list<string>
     */
    private static function normalizeStaticNameList($value): array
    {
        if (! is_array($value)) {
            return [];
        }

        $names = [];

        foreach ($value as $item) {
            if (! is_string($item)) {
                continue;
            }

            $name = strtolower(trim($item));
            if ($name === '') {
                continue;
            }

            $names[$name] = $name;
        }

        return array_values($names);
    }

    /**
     * @param  array<string, Tool>  $toolByName
     * @param  list<string>  $availableToolNames
     * @return array{tools:list<Tool>,audit:array<string,mixed>}
     */
    private function guestOnlyResolution(array $toolByName, array $availableToolNames): array
    {
        $allowed = self::normalizeStaticNameList(self::GUEST_ALLOWED_TOOLS);
        $allowedSet = array_fill_keys($allowed, true);

        $exposed = [];
        foreach ($allowed as $name) {
            if (isset($toolByName[$name])) {
                $exposed[] = $name;
            }
        }

        $resolvedTools = [];
        foreach ($exposed as $name) {
            $resolvedTools[] = $toolByName[$name];
        }

        $removedByAuthGate = [];
        foreach ($availableToolNames as $name) {
            if (! isset($allowedSet[$name])) {
                $removedByAuthGate[] = $name;
            }
        }

        return [
            'tools' => $resolvedTools,
            'audit' => [
                'policyApplied' => false,
                'authRestricted' => true,
                'sessionAuthenticated' => false,
                'autopilotId' => null,
                'availableTools' => $availableToolNames,
                'exposedTools' => $exposed,
                'allowlist' => $allowed,
                'denylist' => [],
                'removedByAllowlist' => [],
                'removedByDenylist' => [],
                'removedByAuthGate' => $removedByAuthGate,
            ],
        ];
    }
}
