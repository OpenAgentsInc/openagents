<?php

use App\Models\User;
use Illuminate\Http\Client\Request as HttpRequest;
use Illuminate\Support\Facades\Http;

beforeEach(function () {
    config()->set('runtime.elixir.base_url', 'http://runtime.internal');
    config()->set('runtime.elixir.signing_key', 'runtime-signing-key');
    config()->set('runtime.elixir.signing_key_id', 'runtime-v1');
    config()->set('runtime.elixir.signature_ttl_seconds', 60);
    config()->set('runtime.elixir.max_retries', 0);
    config()->set('runtime.elixir.retry_backoff_ms', 1);
    config()->set('runtime.elixir.skills_tool_specs_path', '/internal/v1/skills/tool-specs');
    config()->set('runtime.elixir.skills_skill_specs_path', '/internal/v1/skills/skill-specs');
    config()->set('runtime.elixir.skills_publish_path_template', '/internal/v1/skills/skill-specs/{skill_id}/{version}/publish');
    config()->set('runtime.elixir.skills_release_path_template', '/internal/v1/skills/releases/{skill_id}/{version}');
});

test('runtime skill registry api proxies list and upsert calls', function () {
    $user = User::factory()->create();

    Http::fake([
        'http://runtime.internal/internal/v1/skills/tool-specs' => Http::sequence()
            ->push(['data' => [['tool_id' => 'github.primary', 'version' => 1]]], 200)
            ->push(['data' => ['tool_id' => 'github.custom', 'version' => 1, 'state' => 'validated']], 201),
    ]);

    $listResponse = $this->actingAs($user)->getJson('/api/runtime/skills/tool-specs');
    $listResponse->assertOk()->assertJsonPath('data.0.tool_id', 'github.primary');

    $createResponse = $this->actingAs($user)->postJson('/api/runtime/skills/tool-specs', [
        'tool_spec' => [
            'tool_id' => 'github.custom',
            'version' => 1,
            'tool_pack' => 'coding.v1',
            'name' => 'GitHub Custom',
            'description' => 'custom tool',
            'execution_kind' => 'http',
            'integration_manifest' => [
                'manifest_version' => 'coding.integration.v1',
                'integration_id' => 'github.custom',
                'provider' => 'github',
                'status' => 'active',
                'tool_pack' => 'coding.v1',
                'capabilities' => ['get_issue', 'get_pull_request'],
                'secrets_ref' => ['provider' => 'laravel', 'key_id' => 'intsec_github_custom'],
                'policy' => [
                    'write_operations_mode' => 'enforce',
                    'max_requests_per_minute' => 240,
                ],
            ],
        ],
    ]);

    $createResponse
        ->assertStatus(201)
        ->assertJsonPath('data.tool_id', 'github.custom')
        ->assertJsonPath('data.state', 'validated');

    Http::assertSent(function (HttpRequest $request) use ($user): bool {
        return str_starts_with($request->url(), 'http://runtime.internal/internal/v1/skills/tool-specs')
            && ($request->header('X-OA-USER-ID')[0] ?? null) === (string) $user->id;
    });
});

test('runtime skill registry api publishes and fetches release', function () {
    $user = User::factory()->create();

    Http::fake([
        'http://runtime.internal/internal/v1/skills/skill-specs/github-coding/1/publish' => Http::response([
            'data' => [
                'release_id' => 'skillrel_abc123',
                'skill_id' => 'github-coding',
                'version' => 1,
                'bundle_hash' => str_repeat('a', 64),
            ],
        ], 201),
        'http://runtime.internal/internal/v1/skills/releases/github-coding/1' => Http::response([
            'data' => [
                'skill_id' => 'github-coding',
                'version' => 1,
                'bundle' => ['bundle_format' => 'agent_skills.v1'],
            ],
        ], 200),
    ]);

    $publishResponse = $this->actingAs($user)->postJson('/api/runtime/skills/skill-specs/github-coding/1/publish');

    $publishResponse
        ->assertStatus(201)
        ->assertJsonPath('data.skill_id', 'github-coding');

    $releaseResponse = $this->actingAs($user)->getJson('/api/runtime/skills/releases/github-coding/1');

    $releaseResponse
        ->assertOk()
        ->assertJsonPath('data.bundle.bundle_format', 'agent_skills.v1');
});
