<?php

namespace App\Livewire;

use App\AI\Models;
use Livewire\Component;

class Store extends Component
{
    public $models = Models::MODELS;

    public $agents = [
        [
            'title' => 'Image Generator',
            'description' => 'Lorem ipsum dolor sit amet, consectetur adipiscing elit, sed do eiusmod tempor incididunt.',
        ],
        [
            'title' => 'Research Assistant',
            'description' => 'Lorem ipsum dolor sit amet, consectetur adipiscing elit, sed do eiusmod tempor incididunt.',
        ],
        [
            'title' => 'Brainstorm Bot',
            'description' => 'Lorem ipsum dolor sit amet, consectetur adipiscing elit, sed do eiusmod tempor incididunt.',
        ],
        [
            'title' => 'Style Suggestions',
            'description' => 'Lorem ipsum dolor sit amet, consectetur adipiscing elit, sed do eiusmod tempor incididunt.',
        ],
        [
            'title' => 'PDF Reader',
            'description' => 'Lorem ipsum dolor sit amet, consectetur adipiscing elit, sed do eiusmod tempor incididunt.',
        ],
        [
            'title' => 'Tour Guide',
            'description' => 'Lorem ipsum dolor sit amet, consectetur adipiscing elit, sed do eiusmod tempor incididunt.',
        ],
    ];

    public function render()
    {
        return view('livewire.store')
            ->layout('components.layouts.store');
    }
}
