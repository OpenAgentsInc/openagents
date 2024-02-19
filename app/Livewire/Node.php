<?php

namespace App\Livewire;

use Livewire\Component;

class Node extends Component
{
    public $id;
    public $x;
    public $y;
    public $title;
    public $width;
    public $height;

    public function mount($id, $x, $y, $title, $width, $height)
    {
        $this->id = $id;
        $this->x = $x;
        $this->y = $y;
        $this->title = $title;
        $this->width = $width;
        $this->height = $height;
    }

    public function render()
    {
        return view('livewire.node', [
            'strokeWidth' => 1, // Define the stroke width of the rectangle as an integer
            'radius' => 5, // Radius of the circle
            'circleStrokeWidth' => 2, // Stroke width of the circle
            'circleOffset' => 5 + (2 / 2), // Adjust the circle's center to be on the edge of the node
        ]);
    }
}
