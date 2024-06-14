<?php

use App\Livewire\PopularAgents;
use App\Models\Agent;
use Livewire\Livewire;

it('renders successfully', function () {
    $agent = Agent::factory()->create();
    $agent->thread_count = 8;

    Livewire::test(PopularAgents::class)
        ->assertStatus(200)
        ->assertSeeHtml("<img src=\"{$agent->image_url}\" alt=\"Agent\"")
        ->assertSeeHtml(">{$agent->name}</div>")
        ->assertSeeHtml(">From: {$agent->user->name}</p>")
        ->assertSeeHtml("<span>{$agent->thread_count}</span>");
})->skip();
