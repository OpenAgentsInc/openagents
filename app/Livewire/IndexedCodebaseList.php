<?php

namespace App\Livewire;

use App\AI\GreptileGateway;
use App\Models\Codebase;
use Jantinnerezo\LivewireAlert\LivewireAlert;
use Livewire\Component;

class IndexedCodebaseList extends Component
{
    use LivewireAlert;

    public $codebases = [];

    public $index_link = '';

    public function indexRepo()
    {
        if (strpos($this->index_link, 'https://github.com/') !== 0) {
            $this->alert('error', 'Must be full link');

            return;
        }

        // Extract repository name and owner from the link
        $parts = explode('/', $this->index_link);
        $reponameandowner = $parts[count($parts) - 2].'/'.$parts[count($parts) - 1];

        $gateway = new GreptileGateway();
        $response = $gateway->createRepository($reponameandowner);

        // If response["response"] is "started repo processing", then the repo is being processed
        if ($response['response'] === 'started repo processing') {
            $this->alert('success', 'Repository is being processed');
        } else {
            $this->alert('error', 'Error processing repository');

            return;
        }

    }

    public function mount()
    {
        // If user is not logged in or is not pro, redirect to login page
        if (! auth()->check() || ! auth()->user()->isPro()) {
            return redirect('/');
        }

        $this->codebases = Codebase::all();
    }

    public function render()
    {
        return view('livewire.indexed-codebase-list');
    }
}
