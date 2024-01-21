<?php

namespace App\Markdown;

use League\CommonMark\Node\Node;
use League\CommonMark\Renderer\ChildNodeRendererInterface;
use League\CommonMark\Renderer\NodeRendererInterface;

class DateRenderer implements NodeRendererInterface
{
    public function render(Node $node, ChildNodeRendererInterface $childRenderer)
    {
        if (!($node instanceof DateElement)) {
            throw new \InvalidArgumentException('Incompatible node type: ' . get_class($node));
        }

        $formattedDate = (new \DateTime($node->getDate()))->format('F j, Y');

        // Render using a Blade component
        return view('components.formatted-date', ['date' => $formattedDate])->render();
    }
}
