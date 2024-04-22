<?php

use App\Livewire\PrismDashboard;
use Livewire\Livewire;

it('renders successfully', function () {
    Livewire::test(PrismDashboard::class)
        ->assertStatus(200)
        ->assertSeeHtml('>Recent Payments</div>')
        ->assertSeeHtmlInOrder([
            '<div>Date</div>',
            '<div>ID</div>',
            '<div>Recipient</div>',
            '<div>Amount (₿)</div>',
            '<div>Status</div>',
        ]);
});
