<?php

namespace Database\Factories\CRM;

use App\Models\CRM\Company;
use App\Models\CRM\Contact;
use App\Models\User;
use Illuminate\Database\Eloquent\Factories\Factory;

/**
 * @extends \Illuminate\Database\Eloquent\Factories\Factory<\App\Models\CRM\Note>
 */
class NoteFactory extends Factory
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
            'content' => $this->faker->paragraph(),
            'mentions' => null,
        ];
    }
}