<?php

namespace Database\Factories;

use App\Models\Log;
use Illuminate\Database\Eloquent\Factories\Factory;

/**
 * @extends Factory<Log>
 */
class LogFactory extends Factory
{
    /**
     * Define the model's default state.
     *
     * @return array<string, mixed>
     */
    public function definition(): array
    {
        return [
            'data' => json_encode([
                'user_id' => $this->faker->numberBetween(1, 10),
                'action' => $this->faker->randomElement(['created', 'updated', 'deleted']),
                'description' => $this->faker->sentence,
            ]),
        ];
    }
}
