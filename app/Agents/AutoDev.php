<?php

namespace App\Agents;

use App\Agents\Modules\Actions;
use App\Agents\Modules\Critic;
use App\Agents\Modules\Curriculum;
use App\Agents\Modules\Environment;
use App\Agents\Modules\Memory;
use App\Agents\Modules\Reflection;
use App\Agents\Modules\Skills;
use App\Agents\Modules\Tools;

class AutoDev
{
    // Agent modules
    private $actions;
    private $critic;
    private $curriculum;
    private $environment;
    private $memory;
    private $reflection;
    private $skills;
    private $tools;

    public function __construct($fullRepo = "OpenAgentsInc/openagents")
    {
        // Initialize agent modules
        $this->actions = new Actions();
        $this->critic = new Critic();
        $this->curriculum = new Curriculum();
        $this->environment = new Environment($fullRepo);
        $this->memory = new Memory();
        $this->reflection = new Reflection();
        $this->skills = new Skills();
        $this->tools = new Tools();
    }

    public function run()
    {
        // Observe the environment
        $envSummary = $this->environment->getSummary();

        // Build a curriculum (reading issue(s), inferring user intent) and get next task
        $task = $this->curriculum->getNextTask();

        // Load our skills and tools
        $skills = $this->skills->getSkills();
        $tools = $this->tools->getTools();

        // Generate the code
        $code = $this->actions->generateCode($task, $skills, $tools);

        // Take action in the environment
        [$state, $feedback, $errors] = $this->environment->step($code);
    }
}
