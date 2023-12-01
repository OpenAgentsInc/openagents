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
    public function it_can_create_a_memory()
    {
        $memory = Memory::factory()->create();

        $this->assertDatabaseHas('memories', [
            'id' => $memory->id,
        ]);
    }

    /** @test */
    public function it_can_update_a_memory()
    {
        $memory = Memory::factory()->create();

        $updatedMemory = Memory::factory()->make();

        $memory->update($updatedMemory->toArray());

        $this->assertDatabaseHas('memories', [
            'id' => $memory->id,
            'title' => $updatedMemory->title,
            'description' => $updatedMemory->description,
            'priority' => $updatedMemory->priority,
        ]);
    }

    /** @test */
    public function it_can_delete_a_memory()
    {
        $memory = Memory::factory()->create();

        $memory->delete();

        $this->assertDatabaseMissing('memories', [
            'id' => $memory->id,
        ]);
    }

    /** @test */
    public function it_can_show_a_memory()
    {
        $memory = Memory::factory()->create();

        $this->get(route('memories.show', $memory->id))
            ->assertOk()
            ->assertJson([
                'id' => $memory->id,
                'title' => $memory->title,
                'description' => $memory->description,
                'priority' => $memory->priority,
            ]);
    }
}