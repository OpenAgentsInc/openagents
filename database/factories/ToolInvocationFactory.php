<?php

namespace Database\Factories;

use Illuminate\Database\Eloquent\Factories\Factory;

/**
 * @extends \Illuminate\Database\Eloquent\Factories\Factory<\App\Models\ToolInvocation>
 */
class ToolInvocationFactory extends Factory
{
    /**
     * Define the model's default state.
     *
     * @return array<string, mixed>
     */
    public function definition(): array
    {
        return [
            'tool_name' => $this->faker->randomElement(['view_file', 'create_file', 'rewrite_file', 'delete_file', 'view_hierarchy', 'scrape_webpage']),
            'input' => ['path' => 'example/path.txt'],
            'output' => null,
            'status' => 'pending',
        ];
    }
}