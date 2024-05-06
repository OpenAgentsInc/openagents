<?php

namespace App\Services;

use App\AI\Models;
use App\Models\Agent;

class SharedContextService
{
    private $selectedModel;

    private $selectedAgent;

    public function initializeAgentAndModelContext($thread)
    {
        if (request()->query('model')) {
            session()->put('selectedModel', request()->query('model'));
        }

        if (request()->query('agent')) {
            session()->put('selectedAgent', request()->query('agent'));
            $agent = Agent::find(request()->query('agent'));
            if ($agent) {
                $this->selectedAgent = [
                    'id' => $agent->id,
                    'name' => $agent->name,
                    'description' => $agent->about,
                    'instructions' => $agent->prompt,
                    'image' => $agent->image_url,
                ];
            }
        }

        if (! empty($thread)) {
            $this->selectedModel = Models::getModelForThread($thread);
        }
    }

    public function getSelectedModel()
    {
        return $this->selectedModel;
    }

    public function getSelectedAgent()
    {
        return $this->selectedAgent;
    }
}
