<?php

namespace App\Livewire;

use App\AI\GreptileGateway;
use Livewire\Component;

class IndexedCodebaseList extends Component
{
    public function mount()
    {
        // If user is not logged in or is not pro, redirect to login page
        if (! auth()->check() || ! auth()->user()->isPro()) {
            return redirect('/');
        }

        $gateway = new GreptileGateway();
        $info = $gateway->getRepository();

        dd($info);
    }

    public function render()
    {
        return view('livewire.indexed-codebase-list');
    }
}
