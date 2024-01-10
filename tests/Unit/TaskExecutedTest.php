<?php

use App\Models\Conversation;
use App\Models\Task;
use App\Models\TaskExecuted;

it('belongs to a task', function () {
    $task_executed = TaskExecuted::factory()->create();
    expect($task_executed->task)->toBeInstanceOf(Task::class);
});

it('may belong to a conversation', function () {
    $task_executed = TaskExecuted::factory()->create();
    expect($task_executed->conversation)->toBeInstanceOf(Conversation::class);

    $task_executed2 = TaskExecuted::factory()->create([
        'conversation_id' => null
    ]);
    expect($task_executed2->conversation)->toBeNull();
});
