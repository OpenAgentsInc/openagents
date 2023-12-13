<?php

namespace App\Services;

use GitHub;

class Auditor
{
    public $owner;
    public $repo;

    public function __construct($owner = "OpenAgentsInc", $repo = "openagents")
    {
        $this->owner = $owner;
        $this->repo = $repo;
    }

    // Get repo info
    public function getRepo()
    {
        return GitHub::repo()->show($this->owner, $this->repo);
    }

    // Get file contents of folder
    public function getFolderContents($path = null)
    {
        return GitHub::repo()->contents()->show($this->owner, $this->repo, $path);
    }

    // Begin audit job
    public function audit()
    {

    }
}
