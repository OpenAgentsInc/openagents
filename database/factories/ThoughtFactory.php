<?php

namespace Database\Factories;

use App\Models\Agent;
use App\Models\Brain;
use App\Services\Embedder;
use Illuminate\Database\Eloquent\Factories\Factory;

/**
 * @extends \Illuminate\Database\Eloquent\Factories\Factory<\App\Models\Thought>
 */
class ThoughtFactory extends Factory
{
    /**
     * Define the model's default state.
     *
     * @return array<string, mixed>
     */
    public function definition(): array
    {
        return [
            'agent_id' => Agent::factory(),
            'brain_id' => Brain::factory(),
            'body' => $this->faker->sentence,
            'embedding' => Embedder::createFakeEmbedding()
        ];
    }
}
