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
    private $actions;
    private $critic;
    private $curriculum;
    private $environment;
    private $memory;
    private $reflection;
    private $skills;
    private $tools;

    public function __construct()
    {
        $this->actions = new Actions();
        $this->critic = new Critic();
        $this->curriculum = new Curriculum();
        $this->environment = new Environment();
        $this->memory = new Memory();
        $this->reflection = new Reflection();
        $this->skills = new Skills();
        $this->tools = new Tools();
    }

    public function run()
    {

    }
}
