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
            ['id' => 1, 'x' => 100, 'y' => 50, 'width' => 300, 'height' => 200, 'title' => 'Start'],
            ['id' => 2, 'x' => 600, 'y' => 100, 'width' => 300, 'height' => 200, 'title' => 'End'],
        ];

        // Assuming each edge connects the centers of the right side of one node to the left side of the next node
        $this->edges = [
            ['from' => 1, 'to' => 2]
        ];
    }

    public function render()
    {
        return view('livewire.graph');
    }
}
