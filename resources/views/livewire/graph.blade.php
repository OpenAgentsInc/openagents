<div class="my-12 w-full h-[500px] border border-offblack bg-black relative">
    @foreach($nodes as $node)
        <x-node :id="$node['id']" :x="$node['x']" :y="$node['y']" :title="$node['title']" />
    @endforeach

    @foreach($edges as $edge)
        @php
            $fromNode = collect($nodes)->firstWhere('id', $edge['from']);
            $toNode = collect($nodes)->firstWhere('id', $edge['to']);
            // Assuming the edge connects the right-middle of 'from' node to the left-middle of 'to' node
            $fromX = $fromNode['x'] + $fromNode['width'];
            $fromY = $fromNode['y'] + $fromNode['height'] / 2;
            $toX = $toNode['x'];
            $toY = $toNode['y'] + $toNode['height'] / 2;
        @endphp
        <!-- Edge is a line drawn with SVG -->
        <svg style="position: absolute; top: 0; left: 0; width: 100%; height: 100%;">
            <line x1="{{ $fromX }}" y1="{{ $fromY }}" x2="{{ $toX }}" y2="{{ $toY }}" stroke="white" stroke-width="2" />
        </svg>
    @endforeach
</div>
