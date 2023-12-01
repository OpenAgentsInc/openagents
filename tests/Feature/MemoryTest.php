<?php

namespace Tests\Feature;

use App\Models\Memory;
use Illuminate\Foundation\Testing\RefreshDatabase;
use Illuminate\Foundation\Testing\WithFaker;
use Tests\TestCase;

class MemoryTest extends TestCase
{
    use RefreshDatabase;

    /** @test */
    public function it_can_get_all_memories()
    {
        $memories = Memory::factory()->count(5)->create();

        $response = $this->getJson('/api/memories');

        $response->assertStatus(200)
            ->assertJsonCount(5)
            ->assertJson($memories->toArray());
    }

    /** @test */
    public function it_can_create_a_memory()
    {
        $memory = Memory::factory()->make();

        $response = $this->postJson('/api/memories', $memory->toArray());

        $response->assertStatus(201)
            ->assertJson($memory->toArray());

        $this->assertDatabaseHas('memories', $memory->toArray());
    }

    /** @test */
    public function it_can_get_a_single_memory()
    {
        $memory = Memory::factory()->create();

        $response = $this->getJson('/api/memories/' . $memory->id);

        $response->assertStatus(200)
            ->assertJson($memory->toArray());
    }

    /** @test */
    public function it_can_update_a_memory()
    {
        $memory = Memory::factory()->create();

        $updatedMemory = Memory::factory()->make();

        $response = $this->putJson('/api/memories/' . $memory->id, $updatedMemory->toArray());

        $response->assertStatus(200)
            ->assertJson($updatedMemory->toArray());

        $this->assertDatabaseHas('memories', $updatedMemory->toArray());
    }

    /** @test */
    public function it_can_delete_a_memory()
    {
        $memory = Memory::factory()->create();

        $response = $this->deleteJson('/api/memories/' . $memory->id);

        $response->assertStatus(204);

        $this->assertDatabaseMissing('memories', $memory->toArray());
    }
}