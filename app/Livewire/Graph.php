<?php

namespace App\Livewire;

use Livewire\Component;

class Graph extends Component
{
    public $nodes = [];
    public $edges = [];

    public function mount()
    {
        // Initialize your nodes and edges here
        $this->nodes = [
            ['id' => 1, 'x' => 150, 'y' => 100, 'width' => 220, 'height' => 150, 'title' => 'Start'],
            ['id' => 2, 'x' => 450, 'y' => 125, 'width' => 220, 'height' => 150, 'title' => 'Middle'],
            ['id' => 3, 'x' => 750, 'y' => 150, 'width' => 220, 'height' => 150, 'title' => 'End'],
        ];

        // Assuming each edge connects the centers of the right side of one node to the left side of the next node
        $this->edges = [
            ['from' => 1, 'to' => 2],
            ['from' => 2, 'to' => 3],
        ];
    }

    public function render()
    {
        return view('livewire.graph');
    }
}
