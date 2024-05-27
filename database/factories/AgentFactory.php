<?php

namespace Database\Factories;

use App\Enums\Currency;
use App\Models\Agent;
use App\Models\Thread;
use App\Models\User;
use Illuminate\Database\Eloquent\Factories\Factory;

/**
 * @extends Factory<Thread>
 */
class AgentFactory extends Factory
{
    /**
     * Define the model's default state.
     *
     * @return array<string, mixed>
     */
    public function definition(): array
    {
        return [
            'name' => $this->faker->name,
            'user_id' => User::factory(),
        ];
    }

    public function withBalance(int $amount, Currency $currency)
    {
        return $this->afterCreating(function (Agent $agent) use ($amount, $currency) {
            $agent->newBalance($amount, $currency);
        });
    }
}
