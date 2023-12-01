<?php

namespace Tests\Feature;

use App\Models\Memory;
use Illuminate\Foundation\Testing\RefreshDatabase;
use Illuminate\Foundation\Testing\WithFaker;
use Tests\TestCase;

class MemoryFeatureTest extends TestCase
{
    use RefreshDatabase, WithFaker;

    /** @test */
    public function test_it_can_get_all_memories()
    {
        $memories = Memory::factory()->count(3)->create();

        $response = $this->get('/api/memories');

        $response->assertStatus(200)
            ->assertJson($memories->toArray());
    }

    /** @test */
    public function test_it_can_create_a_memory()
    {
        $data = [
            'title' => $this->faker->sentence,
            'description' => $this->faker->paragraph,
            'date' => $this->faker->date,
            'location' => $this->faker->address,
        ];

        $response = $this->post('/api/memories', $data);

        $response->assertStatus(201)
            ->assertJson($data);
    }

    /** @test */
    public function test_it_can_update_a_memory()
    {
        $memory = Memory::factory()->create();

        $data = [
            'title' => $this->faker->sentence,
            'description' => $this->faker->paragraph,
            'date' => $this->faker->date,
            'location' => $this->faker->address,
        ];

        $response = $this->put('/api/memories/' . $memory->id, $data);

        $response->assertStatus(200)
            ->assertJson($data);
    }

    /** @test */
    public function test_it_can_delete_a_memory()
    {
        $memory = Memory::factory()->create();

        $response = $this->delete('/api/memories/' . $memory->id);

        $response->assertStatus(200)
            ->assertJson(['message' => 'Memory deleted successfully.']);
    }
}