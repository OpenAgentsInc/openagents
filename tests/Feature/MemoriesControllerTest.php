<?php

namespace Tests\Feature;

use Illuminate\Foundation\Testing\RefreshDatabase;
use Illuminate\Foundation\Testing\WithFaker;
use Tests\TestCase;

class MemoriesControllerTest extends TestCase
{
    use RefreshDatabase;

    public function test_create_memory()
    {
        $response = $this->post('/memories', [
            'title' => 'Test Memory',
            'description' => 'This is a test memory',
            'date' => '2020-01-01',
            'location' => 'Test Location',
            'image' => 'test.jpg'
        ]);

        $response->assertStatus(201);
        $this->assertDatabaseHas('memories', [
            'title' => 'Test Memory',
            'description' => 'This is a test memory',
            'date' => '2020-01-01',
            'location' => 'Test Location',
            'image' => 'test.jpg'
        ]);
    }

    public function test_read_memory()
    {
        $memory = factory(\App\Memory::class)->create();

        $response = $this->get('/memories/' . $memory->id);
$response->assertStatus(201);
$response->assertJson([
            'title' => $memory->title,
            'description' => $memory->description,
            'date' => $memory->date,
            'location' => $memory->location,
            'image' => $memory->image
        ]);
    }

    public function test_update_memory()
    {
        $memory = factory(\App\Memory::class)->create();

        $response = $this->put('/memories/' . $memory->id, [
            'title' => 'Updated Memory',
            'description' => 'This is an updated memory',
            'date' => '2020-02-02',
            'location' => 'Updated Location',
            'image' => 'updated.jpg'
        ]);

        $response->assertStatus(200);
        $this->assertDatabaseHas('memories', [
            'id' => $memory->id,
            'title' => 'Updated Memory',
            'description' => 'This is an updated memory',
            'date' => '2020-02-02',
            'location' => 'Updated Location',
            'image' => 'updated.jpg'
        ]);
    }

    public function test_delete_memory()
    {
        $memory = factory(\App\Memory::class)->create();

        $response = $this->delete('/memories/' . $memory->id);

        $response->assertStatus(200);
        $this->assertDatabaseMissing('memories', [
            'id' => $memory->id
        ]);
    }
}