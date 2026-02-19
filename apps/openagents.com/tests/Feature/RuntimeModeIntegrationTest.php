<?php

use App\AI\Agents\AutopilotAgent;
use App\AI\Runtime\RuntimeClient;
use App\Models\User;
use Illuminate\Support\Facades\DB;
use Illuminate\Support\Str;
use Laravel\Ai\Ai;
use Symfony\Component\HttpFoundation\StreamedResponse;

test('runtime mode preserves SSE continuity with start delta finish and DONE', function () {
    config()->set('runtime.driver', 'elixir');

    $user = User::factory()->create();
    $conversationId = createRuntimeConversation($user->id);

    $runtimeSpy = fakeRuntimeStreamClient(
        "data: {\"type\":\"start\"}\n\n"
        ."data: {\"type\":\"text-delta\",\"id\":\"rt_1\",\"delta\":\"runtime hello\"}\n\n"
        ."data: {\"type\":\"finish\"}\n\n"
        ."data: [DONE]\n\n"
    );

    app()->instance(RuntimeClient::class, $runtimeSpy);

    $response = $this->actingAs($user)->postJson('/api/chat?conversationId='.$conversationId, [
        'messages' => [
            ['id' => 'm1', 'role' => 'user', 'content' => 'hello runtime continuity'],
        ],
    ]);

    $response->assertOk();

    $content = $response->streamedContent();
    expect($content)->toContain('data: {"type":"start"}');
    expect($content)->toContain('runtime hello');
    expect($content)->toContain('data: {"type":"finish"}');
    expect($content)->toContain("data: [DONE]\n\n");

    expect($runtimeSpy->calls)->toHaveCount(1);
    expect(DB::table('runs')->where('thread_id', $conversationId)->count())->toBe(0);
});

test('runtime mode forwards cancellation finish semantics and terminates stream cleanly', function () {
    config()->set('runtime.driver', 'elixir');

    $user = User::factory()->create();
    $conversationId = createRuntimeConversation($user->id);

    $runtimeSpy = fakeRuntimeStreamClient(
        "data: {\"type\":\"start\"}\n\n"
        ."data: {\"type\":\"text-delta\",\"id\":\"rt_cancel\",\"delta\":\"working\"}\n\n"
        ."data: {\"type\":\"finish\",\"finishReason\":\"cancelled\"}\n\n"
        ."data: [DONE]\n\n"
    );

    app()->instance(RuntimeClient::class, $runtimeSpy);

    $response = $this->actingAs($user)->postJson('/api/chat?conversationId='.$conversationId, [
        'messages' => [
            ['id' => 'm1', 'role' => 'user', 'content' => 'simulate cancel'],
        ],
    ]);

    $response->assertOk();
    $content = $response->streamedContent();

    expect($content)->toContain('"finishReason":"cancelled"');
    expect($content)->toContain("data: [DONE]\n\n");
});

test('runtime mode falls back to legacy runtime when runtime client throws', function () {
    config()->set('runtime.driver', 'elixir');
    Ai::fakeAgent(AutopilotAgent::class, ['legacy fallback reply']);

    $user = User::factory()->create();
    $conversationId = createRuntimeConversation($user->id);

    $throwingClient = new class implements RuntimeClient
    {
        public function driverName(): string
        {
            return 'elixir';
        }

        public function streamAutopilotRun(
            \Illuminate\Contracts\Auth\Authenticatable $user,
            string $threadId,
            string $prompt,
            bool $authenticatedSession = true,
        ): StreamedResponse {
            throw new \RuntimeException('runtime unavailable');
        }
    };

    app()->instance(RuntimeClient::class, $throwingClient);

    $response = $this->actingAs($user)->postJson('/api/chat?conversationId='.$conversationId, [
        'messages' => [
            ['id' => 'm1', 'role' => 'user', 'content' => 'trigger runtime fallback'],
        ],
    ]);

    $response->assertOk();
    $content = $response->streamedContent();

    expect($content)->toContain('data: {"type":"finish"');
    expect($content)->toContain("data: [DONE]\n\n");
    expect(DB::table('runs')->where('thread_id', $conversationId)->count())->toBe(1);
});

test('ownership and auth invariants stay enforced in runtime mode', function () {
    config()->set('runtime.driver', 'elixir');

    $owner = User::factory()->create();
    $intruder = User::factory()->create();
    $conversationId = createRuntimeConversation($owner->id);

    $runtimeSpy = fakeRuntimeStreamClient("data: {\"type\":\"start\"}\n\ndata: [DONE]\n\n");
    app()->instance(RuntimeClient::class, $runtimeSpy);

    $this->actingAs($intruder)->postJson('/api/chat?conversationId='.$conversationId, [
        'messages' => [
            ['id' => 'm1', 'role' => 'user', 'content' => 'intruder'],
        ],
    ])->assertNotFound();

    expect($runtimeSpy->calls)->toHaveCount(0);
});

function createRuntimeConversation(int $userId): string
{
    $conversationId = (string) Str::uuid7();

    DB::table('agent_conversations')->insert([
        'id' => $conversationId,
        'user_id' => $userId,
        'title' => 'Runtime mode conversation',
        'created_at' => now(),
        'updated_at' => now(),
    ]);

    return $conversationId;
}

function fakeRuntimeStreamClient(string $payload): RuntimeClient
{
    return new class($payload) implements RuntimeClient
    {
        /** @var array<int, array<string, mixed>> */
        public array $calls = [];

        public function __construct(private readonly string $payload) {}

        public function driverName(): string
        {
            return 'elixir';
        }

        public function streamAutopilotRun(
            \Illuminate\Contracts\Auth\Authenticatable $user,
            string $threadId,
            string $prompt,
            bool $authenticatedSession = true,
        ): StreamedResponse {
            $this->calls[] = [
                'user_id' => (int) $user->getAuthIdentifier(),
                'thread_id' => $threadId,
                'prompt' => $prompt,
                'authenticated_session' => $authenticatedSession,
            ];

            return response()->stream(function (): void {
                echo $this->payload;
            }, 200, [
                'Content-Type' => 'text/event-stream',
                'x-vercel-ai-ui-message-stream' => 'v1',
            ]);
        }
    };
}
