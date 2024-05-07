<?php

use App\Livewire\Blog;
use Livewire\Livewire;

it('renders successfully', function () {
    Livewire::test(Blog::class)
        ->assertStatus(200);
});
