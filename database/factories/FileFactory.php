<?php

namespace Database\Factories;

use App\Models\Project;
use Illuminate\Database\Eloquent\Factories\Factory;

/**
 * @extends \Illuminate\Database\Eloquent\Factories\Factory<\App\Models\File>
 */
class FileFactory extends Factory
{
    /**
     * Define the model's default state.
     *
     * @return array<string, mixed>
     */
    public function definition(): array
    {
        return [
            'name' => $this->faker->word . '.' . $this->faker->fileExtension(),
            'path' => 'uploads/' . $this->faker->md5 . '.' . $this->faker->fileExtension(),
            'content' => $this->faker->paragraphs(3, true),
            'project_id' => Project::factory(),
        ];
    }
}