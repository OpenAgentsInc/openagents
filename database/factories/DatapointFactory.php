<?php

namespace Database\Factories;

use App\Models\Brain;
use Illuminate\Database\Eloquent\Factories\Factory;

/**
 * @extends \Illuminate\Database\Eloquent\Factories\Factory<\App\Models\Datapoint>
 */
class DatapointFactory extends Factory
{
    /**
     * Define the model's default state.
     *
     * @return array<string, mixed>
     */
    public function definition(): array
    {
        return [
            'brain_id' => Brain::factory(),
            'data' => $this->faker->sentence
        ];
    }
}
