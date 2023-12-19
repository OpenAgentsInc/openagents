<?php

namespace App\Services;

use GitHub;
use App\Events\StepCreated;
use App\Jobs\StartAudit;
use App\Models\Agent;
use App\Models\Run;
use App\Models\Step;
use App\Models\Task;
use App\Models\User;

class Auditor
{
    public $owner;
    public $repo;

    public function __construct($owner = "OpenAgentsInc", $repo = "openagents")
    {
        $this->owner = $owner;
        $this->repo = $repo;
        $user_id = auth()->user()->id ?? User::factory()->create()->id;
        $this->agent = Agent::create([
            'user_id' => $user_id,
            'name' => $this->owner . '/' . $this->repo,
        ]);
        $this->task = Task::create([
            'agent_id' => $this->agent->id,
            'description' => 'Auditor analyzes a GitHub repo',
        ]);
        $this->run = Run::create([
            'agent_id' => $this->agent->id,
            'task_id' => $this->task->id,
            'description' => 'Auditor analyzes a GitHub repo',
            'status' => 'pending',
            'amount' => 0
        ]);
    }

    public function audit()
    {
        $this->getRepo();
        $this->getFolderContents();
        $this->reflect();
    }

    public function reflect()
    {
        $thought = $this->agent->reflect();
        $this->recordStep('Reflect', null, $thought);
    }

    // Get repo info
    public function getRepo()
    {
        $info = GitHub::repo()->show($this->owner, $this->repo);
        $this->recordStep('Get repo data', null, $info);
        return $info;
    }

    // Get file contents of folder
    public function getFolderContents($path = null)
    {
        $contents = GitHub::repo()->contents()->show($this->owner, $this->repo, $path);
        $this->recordStep('Get folder contents', $path, $contents);
        return $contents;
    }

    // Begin audit job
    public function dispatchAuditJob()
    {
        StartAudit::dispatch($this);
    }

    public function log($wat)
    {
        // Only dump if env is local
        if (app()->environment('local')) {
            dump($wat);
        } else {
            // Use regular Laravel log
            \Log::info($wat);
        }
    }

    public function recordStep($description, $input, $output)
    {
        try {
            $step = Step::create([
                'agent_id' => $this->agent->id,
                'run_id' => $this->run->id,
                'description' => $description,
                'input' => json_encode($input),
                'output' => json_encode($output),
            ]);

            StepCreated::dispatch($step);
        } catch (\Exception $e) {
            $this->log("Failed to record step: " . $e->getMessage());
            return [
                'status' => 'error',
                'message' => $e->getMessage(),
            ];
        }

        return [
            'status' => 'success',
            'step' => $step,
        ];
    }
}
