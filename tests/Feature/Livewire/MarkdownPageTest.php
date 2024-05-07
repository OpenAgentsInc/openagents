<?php

use App\Livewire\MarkdownPage;
use Livewire\Livewire;

it('renders successfully', function () {
    Livewire::test(MarkdownPage::class)
        ->assertStatus(200);
});
