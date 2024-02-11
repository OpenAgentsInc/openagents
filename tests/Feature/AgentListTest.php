<?php

use App\Models\Agent;
use App\Models\User;

test("user sees list of their agents", function () {

    $user = User::factory()->create();
    $agent = Agent::factory()->create(['user_id' => $user->id]);

    $this->actingAs($user);

    $response = $this->get(route('agents.index'))
        ->assertOk();

    // assert we see the agent name
    // $this->assertSee

    // expect($response)->status(200);




    expect($response->content())->toContain($agent->name);

});
