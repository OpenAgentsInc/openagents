<?php

use App\Enums\Currency;
use App\Livewire\AgentCard;
use App\Models\Agent;
use App\Models\User;
use App\Services\PaymentService;
use Livewire\Livewire;

it('renders successfully', function () {
    $agent = Agent::factory()->create();
    $agent->about = 'About this agent';
    $agent->thread_count = 8;

    Livewire::test(AgentCard::class, ['agent' => $agent])
        ->assertStatus(200)
        ->assertSeeHtml("<img src=\"{$agent->image_url}\" alt=\"Agent\"")
        ->assertSeeHtml(">{$agent->name}</div>")
        ->assertSeeHtml(">From: {$agent->user->name}</p>")
        ->assertSeeHtml(">{$agent->about}</p>")
        ->assertSeeHtml("<span>{$agent->sats_earned}</span>")
        ->assertSeeHtml("<span>{$agent->thread_count}</span>");
});

it('renders correct sats_earned based on payments', function () {
    $agent = Agent::factory()->create();
    $agent->about = 'About this agent';
    $agent->thread_count = 8;

    $agent = Agent::factory()->withBalance(800000, Currency::BTC)->create();
    $user = User::factory()->withBalance(300000, Currency::BTC)->create();
    $this->actingAs($user);

    $payService = new PaymentService();
    $res = $payService->payAgentForMessage($agent->id, 8);

    expect($res)->toBeTrue();

    Livewire::test(AgentCard::class, ['agent' => $agent])
        ->assertStatus(200)
        ->assertSeeHtml("<img src=\"{$agent->image_url}\" alt=\"Agent\"")
        ->assertSeeHtml(">{$agent->name}</div>")
        ->assertSeeHtml(">From: {$agent->user->name}</p>")
        ->assertSeeHtml(">{$agent->about}</p>")
        ->assertSeeHtml("<span>{$agent->sats_earned}</span>")
        ->assertSeeHtml("<span>{$agent->thread_count}</span>");
});
