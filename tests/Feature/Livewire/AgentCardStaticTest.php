<?php

use App\Livewire\AgentCardStatic;
use App\Models\Agent;
use Livewire\Livewire;

it('renders static card successfully', function () {
    $agent = Agent::factory()->create();
    $agent->about = 'About this agent';
    $agent->sats_earned = 1000;
    $agent->thread_count = 8;

    Livewire::test(AgentCardStatic::class, ['agent' => $agent])
        ->assertStatus(200)
        ->assertSeeHtml("<img src=\"{$agent->image_url}\" alt=\"Agent\"")
        ->assertSeeHtml(">{$agent->name}</div>")
        ->assertSeeHtml(">From: {$agent->user->name}</p>")
        ->assertSeeHtml(">{$agent->about}</p>")
        ->assertSeeHtml("<span>{$agent->sats_earned}</span>")
        ->assertSeeHtml("<span>{$agent->thread_count}</span>");
})->skip();
