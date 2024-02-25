<?php

use App\Models\Agent;
use App\Models\User;
use function Pest\Laravel\post;

test('can create agent via api', function () {
    $this->actingAs(User::factory()->create());

    post(route('api.agents.store'), [
        'name' => 'Test Agent',
        'description' => 'This is a test agent',
    ])
        ->assertStatus(201)
        ->assertJson([
            'name' => 'Test Agent',
            'description' => 'This is a test agent',
        ]);
});
