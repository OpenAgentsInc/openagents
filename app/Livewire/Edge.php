<?php

namespace App\Livewire;

use Livewire\Component;

class Edge extends Component
{
    public $from;
    public $to;
    public $nodes;

    public function mount($from, $to, $nodes)
    {
        $fromNode = collect($nodes)->firstWhere('id', $from);
        $toNode = collect($nodes)->firstWhere('id', $to);

        // Logic to calculate edge positions based on node dimensions
        $this->from = [
            'x' => $fromNode['x'] + $fromNode['width'] - 2, // Right edge of 'from' node
            'y' => $fromNode['y'] + ($fromNode['height'] / 2), // Vertical center of 'from' node
        ];

        $this->to = [
            'x' => $toNode['x'] + 2, // Left edge of 'to' node
            'y' => $toNode['y'] + ($toNode['height'] / 2), // Vertical center of 'to' node
        ];
    }

    protected function findNodeById($id, $nodes)
    {
        foreach ($nodes as $node) {
            if ($node['id'] == $id) {
                return $node;
            }
        }

        return null; // Consider handling the case when a node is not found
    }

    public function render()
    {
        return view('livewire.edge');
    }
}
