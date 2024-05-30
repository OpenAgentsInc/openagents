<?php

namespace Database\Factories;

use App\Models\PaymentSource;
use App\Models\User;
use Illuminate\Database\Eloquent\Factories\Factory;

/**
 * @extends Factory<PaymentSource>
 */
class PaymentSourceFactory extends Factory
{
    /**
     * Define the model's default state.
     *
     * @return array<string, mixed>
     */
    public function definition(): array
    {
        return [
            'source_type' => User::class,
            'source_id' => User::factory(),
        ];
    }
}
