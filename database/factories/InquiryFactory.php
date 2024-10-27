<?php

namespace Database\Factories;

use Illuminate\Database\Eloquent\Factories\Factory;

/**
 * @extends \Illuminate\Database\Eloquent\Factories\Factory<\App\Models\Inquiry>
 */
class InquiryFactory extends Factory
{
    /**
     * Define the model's default state.
     *
     * @return array<string, mixed>
     */
    public function definition(): array
    {
        return [
            'inquiry_type' => fake()->randomElement(['general_question', 'request_demo', 'custom_agents', 'bulk_credits', 'other']),
            'email' => fake()->safeEmail(),
            'comment' => fake()->paragraph(),
        ];
    }
}