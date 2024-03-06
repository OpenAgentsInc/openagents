<?php

namespace Tests\Feature\Livewire;

use App\Livewire\Frontpage;
use Livewire\Livewire;
use Tests\TestCase;

class FrontpageTest extends TestCase
{
    /** @test */
    public function renders_successfully()
    {
        Livewire::test(Frontpage::class)
            ->assertStatus(200);
    }
}
