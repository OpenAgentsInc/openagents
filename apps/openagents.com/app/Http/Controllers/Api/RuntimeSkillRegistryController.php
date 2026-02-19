<?php

namespace App\Http\Controllers\Api;

use App\AI\Runtime\RuntimeSkillRegistryClient;
use App\Http\Controllers\Controller;
use Illuminate\Http\JsonResponse;
use Illuminate\Http\Request;

class RuntimeSkillRegistryController extends Controller
{
    public function listToolSpecs(Request $request, RuntimeSkillRegistryClient $client): JsonResponse
    {
        $user = $request->user();
        if (! $user) {
            abort(401);
        }

        $path = (string) config('runtime.elixir.skills_tool_specs_path', '/internal/v1/skills/tool-specs');

        $result = $client->request('GET', $path, null, [
            'user_id' => (int) $user->getAuthIdentifier(),
        ]);

        return $this->fromRuntimeResult($result);
    }

    public function storeToolSpec(Request $request, RuntimeSkillRegistryClient $client): JsonResponse
    {
        $user = $request->user();
        if (! $user) {
            abort(401);
        }

        $validated = $request->validate([
            'tool_spec' => ['required', 'array'],
            'state' => ['nullable', 'string', 'in:draft,validated,published,deprecated'],
        ]);

        $path = (string) config('runtime.elixir.skills_tool_specs_path', '/internal/v1/skills/tool-specs');

        $result = $client->request('POST', $path, $validated, [
            'user_id' => (int) $user->getAuthIdentifier(),
        ]);

        return $this->fromRuntimeResult($result);
    }

    public function listSkillSpecs(Request $request, RuntimeSkillRegistryClient $client): JsonResponse
    {
        $user = $request->user();
        if (! $user) {
            abort(401);
        }

        $path = (string) config('runtime.elixir.skills_skill_specs_path', '/internal/v1/skills/skill-specs');

        $result = $client->request('GET', $path, null, [
            'user_id' => (int) $user->getAuthIdentifier(),
        ]);

        return $this->fromRuntimeResult($result);
    }

    public function storeSkillSpec(Request $request, RuntimeSkillRegistryClient $client): JsonResponse
    {
        $user = $request->user();
        if (! $user) {
            abort(401);
        }

        $validated = $request->validate([
            'skill_spec' => ['required', 'array'],
            'state' => ['nullable', 'string', 'in:draft,validated,published,deprecated'],
        ]);

        $path = (string) config('runtime.elixir.skills_skill_specs_path', '/internal/v1/skills/skill-specs');

        $result = $client->request('POST', $path, $validated, [
            'user_id' => (int) $user->getAuthIdentifier(),
        ]);

        return $this->fromRuntimeResult($result);
    }

    public function publishSkillSpec(string $skillId, string $version, Request $request, RuntimeSkillRegistryClient $client): JsonResponse
    {
        $user = $request->user();
        if (! $user) {
            abort(401);
        }

        $pathTemplate = (string) config('runtime.elixir.skills_publish_path_template', '/internal/v1/skills/skill-specs/{skill_id}/{version}/publish');
        $path = str_replace(['{skill_id}', '{version}'], [$skillId, $version], $pathTemplate);

        $result = $client->request('POST', $path, [], [
            'user_id' => (int) $user->getAuthIdentifier(),
        ]);

        return $this->fromRuntimeResult($result);
    }

    public function showSkillRelease(string $skillId, string $version, Request $request, RuntimeSkillRegistryClient $client): JsonResponse
    {
        $user = $request->user();
        if (! $user) {
            abort(401);
        }

        $pathTemplate = (string) config('runtime.elixir.skills_release_path_template', '/internal/v1/skills/releases/{skill_id}/{version}');
        $path = str_replace(['{skill_id}', '{version}'], [$skillId, $version], $pathTemplate);

        $result = $client->request('GET', $path, null, [
            'user_id' => (int) $user->getAuthIdentifier(),
        ]);

        return $this->fromRuntimeResult($result);
    }

    /**
     * @param  array{ok: bool, status: int|null, body: mixed, error: string|null}  $result
     */
    private function fromRuntimeResult(array $result): JsonResponse
    {
        if ($result['ok'] === true) {
            if (is_array($result['body'])) {
                return response()->json($result['body'], $result['status'] ?? 200);
            }

            return response()->json(['data' => ['raw' => (string) ($result['body'] ?? '')]], $result['status'] ?? 200);
        }

        $status = $result['status'] ?? 502;

        if (is_array($result['body'])) {
            return response()->json($result['body'], $status);
        }

        return response()->json([
            'error' => [
                'code' => 'runtime_skill_registry_failed',
                'message' => (string) ($result['error'] ?? 'runtime skill registry request failed'),
            ],
        ], $status);
    }
}
