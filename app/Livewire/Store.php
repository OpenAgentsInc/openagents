<?php

namespace App\Livewire;

use App\AI\Models;
use Livewire\Component;

class Store extends Component
{
    public $models = Models::MODELS;

    public $agents = [
        [
            'id' => 1,
            'title' => 'Image Generator',
            'description' => 'Lorem ipsum dolor sit amet, consectetur adipiscing elit, sed do eiusmod tempor incididunt.',
        ],
        [
            'id' => 2,
            'title' => 'Research Assistant',
            'description' => 'Lorem ipsum dolor sit amet, consectetur adipiscing elit, sed do eiusmod tempor incididunt.',
        ],
        [
            'id' => 3,
            'title' => 'Brainstorm Bot',
            'description' => 'Lorem ipsum dolor sit amet, consectetur adipiscing elit, sed do eiusmod tempor incididunt.',
        ],
        [
            'id' => 4,
            'title' => 'Style Suggestions',
            'description' => 'Lorem ipsum dolor sit amet, consectetur adipiscing elit, sed do eiusmod tempor incididunt.',
        ],
        [
            'id' => 5,
            'title' => 'PDF Reader',
            'description' => 'Lorem ipsum dolor sit amet, consectetur adipiscing elit, sed do eiusmod tempor incididunt.',
        ],
        [
            'id' => 6,
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
