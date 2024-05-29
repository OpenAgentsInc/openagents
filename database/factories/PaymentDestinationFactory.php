<?php

namespace Database\Factories;

use App\Models\PaymentDestination;
use App\Models\User;
use Illuminate\Database\Eloquent\Factories\Factory;

/**
 * @extends Factory<PaymentDestination>
 */
class PaymentDestinationFactory extends Factory
{
    /**
     * Define the model's default state.
     *
     * @return array<string, mixed>
     */
    public function definition(): array
    {
        return [
            'destination_type' => User::class,
            'destination_id' => User::factory(),
        ];
    }
}
