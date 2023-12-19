<?php

namespace App\Agents;

use App\Models\Agent;

class AutoDev
{
    private $environment;
    private $curriculum;
    private $actions;
    private $critic;
    private $skills;

    public function __construct()
    {
        $this->environment = new Environment();
        $this->curriculum = new Curriculum();
        $this->actions = new Actions();
        $this->critic = new Critic();
        $this->skills = new Skills();
    }

    public function run()
    {
        $agentState = $this->environment->reset();
        while (true) {
            $explorationProgress = $this->curriculum->getExplorationProgress(
                $this->curriculum->getCompletedTasks(),
                $this->curriculum->getFailedTasks()
            );

            $task = $this->curriculum->proposeNextTask($agentState, $explorationProgress);

            $code = null;
            $environmentFeedback = null;
            $executionErrors = null;
            $critique = null;
            $success = false;

            // Try at most 4 rounds before moving on to the next task
            for ($i = 0; $i < 4; $i++) {
                $skills = $this->skills->retrieveSkills($task, $environmentFeedback);
                $code = $this->actions->generateCode(
                    $task,
                    $code,
                    $environmentFeedback,
                    $executionErrors,
                    $critique,
                    $skills
                );

                list($agentState, $environmentFeedback, $executionErrors) = $this->environment->step($code);

                list($success, $critique) = $this->critic->checkTaskSuccess($task, $agentState);

                if ($success) {
                    break;
                }
            }

            if ($success) {
                $this->skills->addSkill($code);
                $this->curriculum->addCompletedTask($task);
            } else {
                $this->curriculum->addFailedTask($task);
            }
        }
    }
}
