<?php

namespace Tests\Feature;

use App\Models\Memory;
use Illuminate\Foundation\Testing\RefreshDatabase;
use Illuminate\Foundation\Testing\WithFaker;
use Tests\TestCase;

class MemoriesControllerTest extends TestCase
{
    use RefreshDatabase;

    public function test_can_create_memory()
    {
        $memory = Memory::factory()->create();

        $response = $this->post('/api/memories', [
            'title' => $memory->title,
            'description' => $memory->description,
            'date' => $memory->date,
            'location' => $memory->location,
            'image' => $memory->image,
        ]);

        $response->assertStatus(201);
        $this->assertDatabaseHas('memories', [
            'title' => $memory->title,
            'description' => $memory->description,
            'date' => $memory->date,
            'location' => $memory->location,
            'image' => $memory->image,
        ]);
    }

    public function test_can_read_memory()
    {
        $memory = Memory::factory()->create();

        $response = $this->get('/api/memories/' . $memory->id);

        $response->assertStatus(200);
        $response->assertJson([
            'title' => $memory->title,
            'description' => $memory->description,
            'date' => $memory->date,
            'location' => $memory->location,
            'image' => $memory->image,
        ]);
    }

    public function test_can_update_memory()
    {
        $memory = Memory::factory()->create();

        $response = $this->put('/api/memories/' . $memory->id, [
            'title' => 'Updated Title',
            'description' => 'Updated Description',
            'date' => '2021-01-01',
            'location' => 'Updated Location',
            'image' => 'updated_image.jpg',
        ]);

        $response->assertStatus(200);
        $this->assertDatabaseHas('memories', [
            'id' => $memory->id,
            'title' => 'Updated Title',
            'description' => 'Updated Description',
            'date' => '2021-01-01',
            'location' => 'Updated Location',
            'image' => 'updated_image.jpg',
        ]);
    }

    public function test_can_delete_memory()
    {
        $memory = Memory::factory()->create();

        $response = $this->delete('/api/memories/' . $memory->id);

        $response->assertStatus(200);
        $this->assertDeleted('memories', [
            'id' => $memory->id,
        ]);
    }
}