<?php

use App\Models\Autopilot;
use App\Models\AutopilotPolicy;
use App\Models\AutopilotProfile;
use App\Models\AutopilotRuntimeBinding;
use App\Models\Message;
use App\Models\Run;
use App\Models\RunEvent;
use App\Models\Thread;
use App\Models\User;
use Illuminate\Support\Facades\DB;
use Illuminate\Support\Facades\Schema;
use Illuminate\Support\Str;

it('persists autopilot scoped runtime records and resolves relations', function () {
    $user = User::factory()->create([
        'email' => 'autopilot-model-owner@openagents.com',
    ]);

    $autopilot = Autopilot::query()->create([
        'owner_user_id' => $user->id,
        'handle' => 'autopilot-model-owner',
        'display_name' => 'Owner Autopilot',
        'status' => 'active',
        'visibility' => 'private',
        'config_version' => 1,
    ]);

    $profile = AutopilotProfile::query()->create([
        'autopilot_id' => $autopilot->id,
        'owner_display_name' => 'Chris',
        'persona_summary' => 'Pragmatic and direct.',
        'principles' => ['clarity', 'rigor'],
        'preferences' => ['style' => 'concise'],
        'onboarding_answers' => ['goal' => 'ship EP212'],
    ]);

    $policy = AutopilotPolicy::query()->create([
        'autopilot_id' => $autopilot->id,
        'model_provider' => 'openrouter',
        'model' => 'moonshotai/kimi-k2.5',
        'tool_allowlist' => ['lightning_l402_fetch', 'lightning_l402_approve'],
        'tool_denylist' => ['admin_only_tool'],
        'l402_require_approval' => true,
        'l402_max_spend_msats_per_call' => 100_000,
        'l402_max_spend_msats_per_day' => 500_000,
        'l402_allowed_hosts' => ['sats4ai.com', 'l402.openagents.com'],
    ]);

    $binding = AutopilotRuntimeBinding::query()->create([
        'autopilot_id' => $autopilot->id,
        'runtime_type' => 'laravel',
        'runtime_ref' => 'openagents.com',
        'is_primary' => true,
        'meta' => ['environment' => 'testing'],
    ]);

    $thread = Thread::query()->create([
        'id' => (string) Str::uuid7(),
        'user_id' => $user->id,
        'autopilot_id' => $autopilot->id,
        'title' => 'Autopilot model thread',
    ]);

    $run = Run::query()->create([
        'id' => (string) Str::uuid7(),
        'thread_id' => $thread->id,
        'user_id' => $user->id,
        'autopilot_id' => $autopilot->id,
        'autopilot_config_version' => $autopilot->config_version,
        'status' => 'completed',
        'model_provider' => 'openrouter',
        'model' => 'moonshotai/kimi-k2.5',
        'usage' => ['inputTokens' => 12, 'outputTokens' => 34],
        'meta' => ['route' => 'openrouter_primary_cf_fallback'],
        'started_at' => now()->subSecond(),
        'completed_at' => now(),
    ]);

    $message = Message::query()->create([
        'id' => (string) Str::uuid7(),
        'thread_id' => $thread->id,
        'run_id' => $run->id,
        'user_id' => $user->id,
        'autopilot_id' => $autopilot->id,
        'role' => 'assistant',
        'content' => 'Autopilot response payload',
        'meta' => ['partCount' => 1],
    ]);

    $runEvent = RunEvent::query()->create([
        'thread_id' => $thread->id,
        'run_id' => $run->id,
        'user_id' => $user->id,
        'autopilot_id' => $autopilot->id,
        'actor_type' => 'autopilot',
        'actor_autopilot_id' => $autopilot->id,
        'type' => 'run.completed',
        'payload' => ['status' => 'ok'],
        'created_at' => now(),
    ]);

    $autopilot = Autopilot::query()
        ->with(['owner', 'profile', 'policy', 'runtimeBindings', 'threads', 'runs', 'messages', 'runEvents'])
        ->findOrFail($autopilot->id);

    expect(substr((string) $autopilot->id, 14, 1))->toBe('7');
    expect(substr((string) $binding->id, 14, 1))->toBe('7');

    expect($autopilot->owner->id)->toBe($user->id);
    expect($user->autopilots()->count())->toBe(1);

    expect($autopilot->profile?->is($profile))->toBeTrue();
    expect($autopilot->policy?->is($policy))->toBeTrue();
    expect($autopilot->runtimeBindings)->toHaveCount(1);

    expect($autopilot->threads->first()?->is($thread))->toBeTrue();
    expect($autopilot->runs->first()?->is($run))->toBeTrue();
    expect($autopilot->messages->first()?->is($message))->toBeTrue();
    expect($autopilot->runEvents->first()?->is($runEvent))->toBeTrue();

    expect($thread->autopilot?->id)->toBe($autopilot->id);
    expect($run->autopilot?->id)->toBe($autopilot->id);
    expect($message->autopilot?->id)->toBe($autopilot->id);
    expect($runEvent->autopilot?->id)->toBe($autopilot->id);
    expect($runEvent->actorAutopilot?->id)->toBe($autopilot->id);
});

it('has expected autopilot tables columns and indexes', function () {
    expect(Schema::hasTable('autopilots'))->toBeTrue();
    expect(Schema::hasTable('autopilot_profiles'))->toBeTrue();
    expect(Schema::hasTable('autopilot_policies'))->toBeTrue();
    expect(Schema::hasTable('autopilot_runtime_bindings'))->toBeTrue();

    expect(Schema::hasColumns('threads', ['autopilot_id']))->toBeTrue();
    expect(Schema::hasColumns('runs', ['autopilot_id', 'autopilot_config_version']))->toBeTrue();
    expect(Schema::hasColumns('messages', ['autopilot_id']))->toBeTrue();
    expect(Schema::hasColumns('run_events', ['autopilot_id', 'actor_type', 'actor_autopilot_id']))->toBeTrue();

    $autopilotIndexes = collect(DB::select("PRAGMA index_list('autopilots')"))->pluck('name');
    expect($autopilotIndexes)->toContain('autopilots_handle_unique');
    expect($autopilotIndexes)->toContain('autopilots_status_index');
    expect($autopilotIndexes)->toContain('autopilots_owner_updated_index');
    expect($autopilotIndexes)->toContain('autopilots_owner_visibility_updated_index');

    $threadIndexes = collect(DB::select("PRAGMA index_list('threads')"))->pluck('name');
    expect($threadIndexes)->toContain('threads_autopilot_id_index');

    $runIndexes = collect(DB::select("PRAGMA index_list('runs')"))->pluck('name');
    expect($runIndexes)->toContain('runs_autopilot_id_index');

    $messageIndexes = collect(DB::select("PRAGMA index_list('messages')"))->pluck('name');
    expect($messageIndexes)->toContain('messages_autopilot_id_index');

    $runEventIndexes = collect(DB::select("PRAGMA index_list('run_events')"))->pluck('name');
    expect($runEventIndexes)->toContain('run_events_autopilot_id_index');
    expect($runEventIndexes)->toContain('run_events_actor_autopilot_id_index');
    expect($runEventIndexes)->toContain('run_events_autopilot_created_index');
    expect($runEventIndexes)->toContain('run_events_autopilot_type_id_index');
    expect($runEventIndexes)->toContain('run_events_actor_type_id_index');
    expect($runEventIndexes)->toContain('run_events_actor_autopilot_id_id_index');
});
