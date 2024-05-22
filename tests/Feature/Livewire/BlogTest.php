<?php

use App\Livewire\Blog;
use Livewire\Livewire;

it('renders successfully', function () {
    Livewire::test(Blog::class)
        ->assertStatus(200)
        ->assertSee('OpenAgents Blog')
        ->assertSee('Introducing the Agent Store')
        ->assertSee('Goodbye ChatGPT')
        ->assertSee('One agent to rule them all');
});
