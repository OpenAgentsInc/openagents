<?php

namespace Database\Factories;

use App\Enums\Currency;
use App\Models\Payment;
use App\Models\User;
use Illuminate\Database\Eloquent\Factories\Factory;

/**
 * @extends Factory<Payment>
 */
class PaymentFactory extends Factory
{
    /**
     * Define the model's default state.
     *
     * @return array<string, mixed>
     */
    public function definition(): array
    {
        return [
            'payer_type' => User::class, // Assuming `User` is the payer. Replace with actual payer models
            'payer_id' => User::factory(),
            'currency' => $this->faker->randomElement(Currency::cases()),
            'amount' => $this->faker->numberBetween(100, 10000), // Example amount range in smallest denomination
            'metadata' => json_encode([]),
            'description' => $this->faker->sentence(),
        ];
    }
}
