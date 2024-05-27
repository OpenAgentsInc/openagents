<?php

namespace Database\Factories;

use App\Enums\Currency;
use App\Models\Agent;
use App\Models\Balance;
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

    public function withBalance(int $amount, Currency $currency): self
    {
        return $this->afterCreating(function (Agent $agent) use ($amount, $currency) {
            Balance::create([
                'holder_type' => Agent::class,
                'holder_id' => $agent->id,
                'currency' => $currency,
                'amount' => $amount,
            ]);
        });
    }
}
