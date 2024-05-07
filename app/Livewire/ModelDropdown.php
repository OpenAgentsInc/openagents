<?php

namespace App\Livewire;

use App\AI\Models;
use App\Models\Agent;
use Livewire\Component;
use Illuminate\Support\Facades\Session;

class ModelDropdown extends Component
{
    public $selectedAgent;

    public $selectedModel;

    public $formattedModelOrAgent = '';

    public $models;

    public $action;

    public $isOpen = false;

    public $picture;

    public $showAgents = false;

    public $agents = [];

    public function mount($selectedAgent, $selectedModel, $models, $action, $showAgents = false)
    {
        $this->selectedAgent = $selectedAgent;
        $this->selectedModel = $selectedModel;

        // If selectedAgent is an array with >=3 elements, it means the user has selected an agent
        if (is_array($selectedAgent) && count($selectedAgent) >= 3) {
            $this->formattedModelOrAgent = $this->selectedAgent['name'];
            $this->picture = $this->selectedAgent['image'];
        } else {
            $this->formattedModelOrAgent = Models::getModelName($this->selectedModel);
            $this->picture = Models::getModelPicture($this->selectedModel);
        }

        // Filter out 'hidden' models - if access is 'hidden'
        $this->models = collect($models)->filter(function ($model) {
            return $model['access'] !== 'hidden';
        })->toArray();

        $this->action = $action;
        $this->showAgents = $showAgents;

        // If user is authed, grab their 5 most recent agents
        if (auth()->check()) {
            $this->agents = auth()->user()->agents()->orderBy('created_at', 'desc')->limit(5)->get()->append('image_url')->toArray();
        }
    }

    public function selectAgent($agent)
    {
        $this->dispatch('select-agent', $agent);
        $agent = Agent::find($agent);
        $this->selectedAgent = [
            'id' => $agent->id,
            'name' => $agent->name,
            'description' => $agent->about,
            'instructions' => $agent->message,
        ];

        $this->formattedModelOrAgent = $agent->name;
        $this->picture = $agent->image_url;

        $this->selectedModel = '';
        session()->forget('selectedModel');
    }

    public function selectAgent($agent_id){
        $agent = Agent::find($agent_id);
        if($agent){
            $this->selectedAgent = [
                'id' => $agent->id,
                'name' => $agent->name,
                'description' => $agent->about,
                'instructions' => $agent->message,
                'image' => $agent->image_url
            ];
            $this->dispatch('select-model', $agent_id);
        }
    }

    public function selectModel($model)
    {
        $this->selectedAgent = '';
        session()->forget('selectedAgent');

        $userAccess = $this->getUserAccess();

        // If the user is not logged in, show the login modal for any model they don't have access to
        if ($userAccess === 'guest' && !$this->hasModelAccess($model, $userAccess)) {
            $this->dispatch('openModal', 'auth.join');

            return;
        }

        // Check if the selected model requires "pro" access
        if ($this->isProModelSelected($model)) {
            // If the user is logged in but not a "pro" user, show the upgrade modal
            if ($userAccess !== 'pro') {
                $this->dispatch('openModal', 'modals.upgrade');

                return;
            }
        }

        // If the user has access to the selected model, update the selected model
        if ($this->hasModelAccess($model, $userAccess)) {
            $this->selectedModel = $model;
            $this->formattedModelOrAgent = Models::getModelName($model);
            $this->dispatch('select-model', $model);
        }

        $this->picture = Models::getModelPicture($this->selectedModel);
    }

    public function getUserAccess()
    {
        return Models::getUserAccess();
    }

    public function hasModelAccess($model, $userAccess)
    {
        return Models::hasModelAccess($model, $userAccess);
    }

    protected function isProModelSelected($model)
    {
        return Models::isProModelSelected($model);
    }

    public function createAgent()
    {
        $userAccess = $this->getUserAccess();
        if ($userAccess === 'guest') {
            $this->dispatch('openModal', 'auth.join');
        } elseif ($this->getUserAccess() !== 'pro') {
            $this->dispatch('openModal', 'modals.upgrade');
        } else {
            $this->redirect(route('agents.create'), true);
        }
    }

    public function toggleDropdown()
    {
        $this->isOpen = !$this->isOpen;
    }

    public function render()
    {
        return view('livewire.model-dropdown');
    }

    public function getModelIndicator($modelKey)
    {
        return Models::getModelIndicator($modelKey, $this->getUserAccess());
    }


    public function getRecentAgent()
    {
        $user_id =  auth()->check() ? auth()->id : Session::getId();
        // Get the three most recent unique agents
        $messages = Message::where('user_id', $user_id)->orWhere('session_id')
            ->where('agent_id', '!=', null)
            ->select('agent_id')
            ->distinct()
            ->orderBy('created_at', 'desc')
            ->take(3)
            ->with('agent')
            // ->pluck('agent_id')
            ->get();

        $agents = [];

        if ($messages) {
            foreach ($messages as $message) {
                $agents[] = [
                    'id' => $message->agent->id,
                    'name' => $message->agent->name,
                    'description' => $message->agent->about,
                    'instructions' => $message->agent->message,
                    'image' => $message->agent->image_url
                ];
            }
        }

        // dd($agents);


        return $agents;
    }
}
