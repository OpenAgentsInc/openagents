<?php

use App\Models\Agent;
use App\Models\Brain;
use App\Models\Conversation;
use App\Models\File;
use App\Models\Step;
use App\Models\StepExecuted;
use App\Models\Task;
use App\Models\TaskExecuted;
use App\Models\Thought;
use App\Models\User;
use Database\Seeders\ConciergeSeeder;

it('can fetch chat task, creating if not exists', function () {
    $agent = Agent::factory()->create();
    $task = $agent->getChatTask();
    $firstTask = Task::first();

    expect($task->id)->toBe($firstTask->id);
});

it('can fetch chat task, returning existing task', function () {
    $agent = Agent::factory()->create();
    $firstTask = Task::create([
        'name' => 'Basic LLM Chat',
        'description' => 'This is the default task for this agent.',
        'agent_id' => $agent->id,
    ]);
    $task = $agent->getChatTask();
    expect($task->id)->toBe($firstTask->id);
});

it('can fetch retrieval task, creating if not exists', function () {
    $agent = Agent::factory()->create();
    $task = $agent->getRetrievalTask();
    $firstTask = Task::first();

    expect($task->id)->toBe($firstTask->id);
});

it('can fetch retrieval task, returning existing task', function () {
    $agent = Agent::factory()->create();
    $firstTask = Task::create([
        'name' => 'LLM Chat With Knowledge Retrieval',
        'description' => 'Chat with LLM using knowledge retrieval.',
        'agent_id' => $agent->id,
    ]);
    $task = $agent->getRetrievalTask();
    expect($task->id)->toBe($firstTask->id);
});

it('can get conversation with current user - if it already exists', function () {
    $user = User::factory()->create();
    $this->actingAs($user);
    $agent = Agent::factory()->create(['user_id' => $user->id]);
    $conversation = Conversation::factory()->create([
        'agent_id' => $agent->id,
        'user_id' => $user->id
    ]);
    expect($agent->getUserConversation()->id)->toBe($conversation->id);
});

it('can get conversation with current user - if it doesnt already exist', function () {
    expect(Conversation::count())->toBe(0);

    $user = User::factory()->create();
    $this->actingAs($user);
    $agent = Agent::factory()->create(['user_id' => $user->id]);

    $conversation = $agent->getUserConversation();

    expect(Conversation::count())->toBe(1);
    expect($conversation->agent_id)->toBe($agent->id);
    expect($conversation->user_id)->toBe($user->id);
});

it('has a balance', function () {
    $agent = Agent::factory()->create();
    expect($agent->balance)->toBe(0);

    $agent = Agent::factory()->create(['balance' => 1000]);
    expect($agent->balance)->toBe(1000);
});

it('hasmany brains', function () {
    $agent = Agent::factory()->create();
    $brain = Brain::factory()->create(['agent_id' => $agent->id]);
    $brain2 = Brain::factory()->create(['agent_id' => $agent->id]);
    expect($agent->brains->count())->toBe(2);
});

it('can run', function () {
    $this->seed(ConciergeSeeder::class);
    // Assert 0 TaskExecuted and StepExecuted
    expect(TaskExecuted::count())->toBe(0);
    expect(StepExecuted::count())->toBe(0);

    $agent = Agent::first();
    $agent->run(["input" => "Does this work?"]);
    // There should be one TaskExecuted and four StepExecuteds

    expect(TaskExecuted::count())->toBe(1);
    expect(StepExecuted::count())->toBe($agent->steps->count());
})->group('integration');

it('belongs to a user', function () {
    $user = User::factory()->create();
    $agent = Agent::factory()->create(['user_id' => $user->id]);

    $this->assertInstanceOf(User::class, $agent->user);
});

it('has a name', function () {
    $agent = Agent::factory()->create(['name' => 'My Agent']);
    expect($agent->name)->toBe('My Agent');
});

it('has a description', function () {
    $agent = Agent::factory()->create(['description' => 'An awesome agent']);
    expect($agent->description)->toBe('An awesome agent');
});

it('has instructions', function () {
    $agent = Agent::factory()->create(['instructions' => 'Do this and that']);
    expect($agent->instructions)->toBe('Do this and that');
});

it('has a welcome message', function () {
    $agent = Agent::factory()->create(['welcome_message' => 'Hey welcome to the agent']);
    expect($agent->welcome_message)->toBe('Hey welcome to the agent');
});

it('has many conversations', function () {
    $user = User::factory()->create();
    $agent = Agent::factory()->create(['user_id' => $user->id]);
    $conversation = Conversation::factory()->create([
      'agent_id' => $agent->id,
      'user_id' => $user->id
    ]);

    $this->assertInstanceOf('Illuminate\Database\Eloquent\Collection', $agent->conversations);
    $this->assertInstanceOf(Conversation::class, $agent->conversations->first());
});

it('has many tasks', function () {
    $agent = Agent::factory()->create();
    $task = Task::factory()->create(['agent_id' => $agent->id]);

    $this->assertInstanceOf('Illuminate\Database\Eloquent\Collection', $agent->tasks);
    $this->assertInstanceOf(Task::class, $agent->tasks->first());
});

it('has many steps', function () {
    $agent = Agent::factory()->create();
    $step = Step::factory()->create(['agent_id' => $agent->id]);

    $this->assertInstanceOf('Illuminate\Database\Eloquent\Collection', $agent->steps);
    $this->assertInstanceOf(Step::class, $agent->steps->first());
});

it('has many files', function () {
    $agent = Agent::factory()->create();
    $file = File::factory()->create(['agent_id' => $agent->id]);

    $this->assertInstanceOf('Illuminate\Database\Eloquent\Collection', $agent->files);
    $this->assertInstanceOf(File::class, $agent->files->first());
});

it('has thoughts', function () {
    $agent = Agent::factory()->create();
    $thought = Thought::factory()->create(['agent_id' => $agent->id]);

    $this->assertInstanceOf('Illuminate\Database\Eloquent\Collection', $agent->thoughts);
    $this->assertInstanceOf(Thought::class, $agent->thoughts->first());
});

it('can create default task', function () {
    $agent = Agent::factory()->create();
    $task = $agent->createDefaultTask();

    expect($task->name)->toBe('Default Task');
    expect($task->description)->toBe('This is the default task for this agent.');
    expect($task->agent_id)->toBe($agent->id);
});
