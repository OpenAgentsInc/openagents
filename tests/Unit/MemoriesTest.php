<?php

namespace Tests\Unit;

use Tests\TestCase;
use Illuminate\Foundation\Testing\RefreshDatabase;

class MemoriesTest extends TestCase
{
    use RefreshDatabase;

    /** @test */
    public function it_can_create_a_memory()
    {
        $memory = factory(\App\Memory::class)->create();

        $this->assertDatabaseHas('memories', [
            'id' => $memory->id,
            'title' => $memory->title,
            'description' => $memory->description,
            'date' => $memory->date,
            'location' => $memory->location,
            'agent_id' => $memory->agent_id,
        ]);
    }

    /** @test */
    public function it_can_update_a_memory()
    {
        $memory = factory(\App\Memory::class)->create();

        $updatedMemory = [
            'title' => 'Updated Title',
            'description' => 'Updated Description',
            'date' => '2020-01-01',
            'location' => 'Updated Location',
            'agent_id' => $memory->agent_id,
        ];

        $this->put('/memories/' . $memory->id, $updatedMemory);

        $this->assertDatabaseHas('memories', $updatedMemory);
    }

    /** @test */
    public function it_can_delete_a_memory()
    {
        $memory = factory(\App\Memory::class)->create();

        $this->delete('/memories/' . $memory->id);

        $this->assertDatabaseMissing('memories', [
            'id' => $memory->id,
            'title' => $memory->title,
            'description' => $memory->description,
            'date' => $memory->date,
            'location' => $memory->location,
            'agent_id' => $memory->agent_id,
        ]);
    }

    /** @test */
    public function it_can_get_all_memories()
    {
        $memories = factory(\App\Memory::class, 5)->create();

        $this->get('/memories');

        $this->assertDatabaseHas('memories', $memories->toArray());
    }

    /** @test */
    public function it_can_get_a_single_memory()
    {
        $memory = factory(\App\Memory::class)->create();

        $this->get('/memories/' . $memory->id);

        $this->assertDatabaseHas('memories', [
            'id' => $memory->id,
            'title' => $memory->title,
            'description' => $memory->description,
            'date' => $memory->date,
            'location' => $memory->location,
            'agent_id' => $memory->agent_id,
        ]);
    }
}