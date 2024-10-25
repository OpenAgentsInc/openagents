<?php

namespace Database\Factories;

use App\Models\Thread;
use App\Models\User;
use Illuminate\Database\Eloquent\Factories\Factory;

/**
 * @extends \Illuminate\Database\Eloquent\Factories\Factory<\App\Models\Message>
 */
class MessageFactory extends Factory
{
    /**
     * Define the model's default state.
     *
     * @return array<string, mixed>
     */
    public function definition(): array
    {
        return [
            'user_id' => User::factory(),
            'thread_id' => Thread::factory(),
            'content' => $this->faker->paragraph,
            'is_system_message' => false,
        ];
    }

    /**
     * Indicate that the message is a system message.
     *
     * @return \Illuminate\Database\Eloquent\Factories\Factory
     */
    public function system()
    {
        return $this->state(function (array $attributes) {
            return [
                'user_id' => null,
                'is_system_message' => true,
            ];
        });
    }
}