<?php

namespace Tests\Feature\Livewire;

use App\Livewire\PayBitcoin;
use Livewire\Livewire;
use Tests\TestCase;

class PayBitcoinTest extends TestCase
{
    /** @test */
    public function renders_successfully()
    {
        Livewire::test(PayBitcoin::class)
            ->assertStatus(200);
    }
}
