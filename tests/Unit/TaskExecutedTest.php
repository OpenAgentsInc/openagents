<?php

use App\Models\Task;
use App\Models\TaskExecuted;

it('belongs to a task', function () {
    $task_executed = TaskExecuted::factory()->create();
    expect($task_executed->task)->toBeInstanceOf(Task::class);
});
