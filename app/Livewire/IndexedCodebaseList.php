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

    public function checkRepo()
    {
        if (strpos($this->index_link, 'https://github.com/') !== 0) {
            $this->alert('error', 'Must be full link');

            return;
        }

        // Extract repository name and owner from the link
        $parts = explode('/', $this->index_link);
        $reponameandowner = $parts[count($parts) - 2].'/'.$parts[count($parts) - 1];

        $repositoryId = 'github:master:'.$reponameandowner;
        //        $repositoryId = 'github:main:'.$reponameandowner;
        //        dd($repositoryId);

        $gateway = new GreptileGateway();
        $response = $gateway->getRepository($repositoryId);

        //        dd($response);

        // Create or update based on the repository ID
        Codebase::create([
            'repository' => $response['repository'],
            'remote' => $response['remote'],
            'branch' => $response['branch'],
            'private' => $response['private'],
            'status' => $response['status'],
            'files_processed' => $response['filesProcessed'],
            'num_files' => $response['numFiles'],
            'sample_questions' => json_encode($response['sampleQuestions']),
            'sha' => $response['sha'],
        ]);

        $this->codebases = Codebase::all();
    }

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
        }

        //        sleep(2);

        // Fetch the info via repositoryId which is in the format github:main:OpenAgentsInc/openagents
        // For now we're assuming main branch
        //        $repositoryId = 'github:main:'.$reponameandowner;
        //        $response = $gateway->getRepository($repositoryId);

        //        dd($response);
        /**
         * array:9 [▼ // app/Livewire/IndexedCodebaseList.php:47
         * "repository" => "openagentsinc/docs"
         * "remote" => "github"
         * "branch" => "main"
         * "private" => false
         * "status" => "processing"
         * "filesProcessed" => 0
         * "numFiles" => 31
         * "sampleQuestions" => []
         * "sha" => ""
         * ]
         */

        // Create a new codebase
        //        Codebase::create([
        //            'repository' => $response['repository'],
        //            'remote' => $response['remote'],
        //            'branch' => $response['branch'],
        //            'private' => $response['private'],
        //            'status' => $response['status'],
        //            'files_processed' => $response['filesProcessed'],
        //            'num_files' => $response['numFiles'],
        //            'sample_questions' => $response['sampleQuestions'],
        //            'sha' => $response['sha'],
        //        ]);

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
