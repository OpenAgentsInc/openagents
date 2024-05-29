<?php

namespace Database\Factories;

use App\Enums\Currency;
use App\Models\Balance;
use App\Models\User;
use Illuminate\Database\Eloquent\Factories\Factory;

/**
 * @extends Factory<Balance>
 */
class BalanceFactory extends Factory
{
    /**
     * Define the model's default state.
     *
     * @return array<string, mixed>
     */
    public function definition(): array
    {
        return [
            'holder_type' => User::class, // Assuming `User` is a model. Replace with actual holder models
            'holder_id' => User::factory(),
            'currency' => $this->faker->randomElement(Currency::cases()),
            'amount' => $this->faker->numberBetween(1000, 1000000), // Example amount range
        ];
    }
}
