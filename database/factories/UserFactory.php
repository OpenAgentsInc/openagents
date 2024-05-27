<?php

namespace Database\Factories;

use App\Enums\Currency;
use App\Models\User;
use Illuminate\Database\Eloquent\Factories\Factory;
use Illuminate\Support\Str;

/**
 * @extends Factory<User>
 */
class UserFactory extends Factory
{
    /**
     * The current password being used by the factory.
     */
    protected static ?string $password;

    /**
     * Define the model's default state.
     *
     * @return array<string, mixed>
     */
    public function definition(): array
    {
        return [
            'name' => fake()->name(),
            'username' => fake()->unique()->userName(),
            'email' => fake()->unique()->safeEmail(),
            'remember_token' => Str::random(10),
            'profile_photo_path' => null,
            'default_model' => null,
        ];
    }

    public function withBalance(int $amount, Currency $currency)
    {
        return $this->afterCreating(function (User $user) use ($amount, $currency) {
            $user->newBalance($amount, $currency);
        });
    }
}
