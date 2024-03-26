<?php

namespace Database\Factories;

use App\Models\Thread;
use Illuminate\Database\Eloquent\Factories\Factory;

/**
 * @extends Factory<Thread>
 */
class ThreadFactory extends Factory
{
    /**
     * Define the model's default state.
     *
     * @return array<string, mixed>
     */
    public function definition(): array
    {
        return [
            'private' => false,
        ];
    }

    /**
     * Indicate that the thread is private.
     *
     * @return $this
     */
    public function private(): self
    {
        return $this->state(function (array $attributes) {
            return [
                'private' => true,
            ];
        });
    }
}
