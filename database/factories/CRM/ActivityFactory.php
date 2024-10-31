<?php

namespace Database\Factories\CRM;

use App\Models\CRM\Company;
use App\Models\CRM\Contact;
use App\Models\User;
use Illuminate\Database\Eloquent\Factories\Factory;

/**
 * @extends \Illuminate\Database\Eloquent\Factories\Factory<\App\Models\CRM\Activity>
 */
class ActivityFactory extends Factory
{
    /**
     * Define the model's default state.
     *
     * @return array<string, mixed>
     */
    public function definition(): array
    {
        return [
            'contact_id' => Contact::factory(),
            'company_id' => Company::factory(),
            'user_id' => User::factory(),
            'type' => $this->faker->randomElement(['email', 'meeting', 'call', 'note']),
            'description' => $this->faker->sentence(),
            'metadata' => ['source' => 'factory'],
            'activity_date' => $this->faker->dateTimeThisMonth(),
        ];
    }
}