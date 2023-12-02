<?php

namespace Tests\Feature;

use App\Models\Memory;
use Illuminate\Foundation\Testing\RefreshDatabase;
use Tests\TestCase;

class MemoriesControllerTest extends TestCase
{
    use RefreshDatabase;

    public function test_can_store_a_new_memory()
    {
        $memory = Memory::factory()->make();
        $response = $this->postJson('/api/memories', $memory->toArray());

        $response->assertStatus(201);
        $this->assertDatabaseHas('memories', $memory->toArray());
    }

    public function test_can_show_an_existing_memory()
    {
        $memory = Memory::factory()->create();

        $response = $this->getJson('/api/memories/' . $memory->id);

        $response->assertStatus(200);
        $response->assertJson($memory->toArray());
    }

    public function test_can_update_a_memory()
    {
        $memory = Memory::factory()->create();
        $newData = Memory::factory()->make();

        $response = $this->putJson('/api/memories/' . $memory->id, $newData->toArray());

        $response->assertStatus(200);
        $this->assertDatabaseHas('memories', $newData->toArray());
    }

    public function test_can_delete_a_memory()
    {
        $memory = Memory::factory()->create();

        $response = $this->deleteJson('/api/memories/' . $memory->id);

        $response->assertStatus(204);
        $this->assertDeleted('memories', ['id' => $memory->id]);
    }

    public function test_can_return_all_memories()
    {
        $memories = Memory::factory()->count(5)->create();

        $response = $this->getJson('/api/memories');

        $response->assertStatus(200);
        $response->assertJson($memories->toArray());
    }
}