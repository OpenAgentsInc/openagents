<div class="my-12 w-full h-[500px] border border-2 border-offblack bg-black relative rounded-xl">

    <!-- Background grid of dots -->
    <svg style="position: absolute; top: 0; left: 0; width: 100%; height: 100%;">
        <defs>
            <pattern id="grid" width="20" height="20" patternUnits="userSpaceOnUse">
                <circle cx="10" cy="10" r="0.5" fill="#2C2C2D" />
            </pattern>
        </defs>
        <rect width="100%" height="100%" fill="url(#grid)" />
    </svg>

    @foreach($nodes as $node)
        <livewire:node :id="$node['id']" :x="$node['x']" :y="$node['y']" :title="$node['title']" :width="$node['width']"
            :height="$node['height']" :key="'node-'.$node['id']">
    @endforeach

    @foreach($edges as $edge)
        @php
            $fromNode = collect($nodes)->firstWhere('id', $edge['from']);
            $toNode = collect($nodes)->firstWhere('id', $edge['to']);

            // Adjust the x position of the 'from' node to the right edge of the node plus half the stroke width
            $fromX = $fromNode['x'] + $fromNode['width'] - 2; // Added 1 for the stroke width
            // Adjust the y position to the vertical center of the node plus half the stroke width
            $fromY = $fromNode['y'] + ($fromNode['height'] / 2);

            // Adjust the x position of the 'to' node to the left edge minus half the stroke width
            $toX = $toNode['x'] + 2; // Subtract 1 for the stroke width
            // Adjust the y position to the vertical center of the node plus half the stroke width
            $toY = $toNode['y'] + ($toNode['height'] / 2);
        @endphp
        <!-- Edge is a line drawn with SVG -->
        <svg style="position: absolute; top: 0; left: 0; width: 100%; height: 100%;">
            <line x1="{{ $fromX }}" y1="{{ $fromY }}" x2="{{ $toX }}" y2="{{ $toY }}" stroke="white" stroke-width="2" />
        </svg>
    @endforeach
</div>
