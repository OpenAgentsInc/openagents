<?php

namespace App\Traits;

trait SelectedModelOrAgentTrait
{
    public $selectedModel = '';

    public $selectedAgent = [];

    public function setSelectedModel($model)
    {
        $this->selectedModel = $model;
    }
}
