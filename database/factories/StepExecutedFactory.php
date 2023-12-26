<?php

namespace Database\Factories;

use App\Models\Step;
use App\Models\TaskExecuted;
use App\Models\User;
use Illuminate\Database\Eloquent\Factories\Factory;

/**
 * @extends \Illuminate\Database\Eloquent\Factories\Factory<\App\Models\StepExecuted>
 */
class StepExecutedFactory extends Factory
{
    /**
     * Define the model's default state.
     *
     * @return array<string, mixed>
     */
    public function definition(): array
    {
        return [
            'order' => 1,
            'status' => 'pending',
            'step_id' => Step::factory(),
            'task_executed_id' => TaskExecuted::factory(),
            'user_id' => User::factory(),
        ];
    }
}
