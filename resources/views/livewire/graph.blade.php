<div class="my-12 w-full h-[500px] border border-offblack bg-black relative">
    @foreach($nodes as $node)
        <x-node :id="$node['id']" :x="$node['x']" :y="$node['y']" :title="$node['title']" />
    @endforeach

    @foreach($edges as $edge)
        @php
            $fromNode = collect($nodes)->firstWhere('id', $edge['from']);
            $toNode = collect($nodes)->firstWhere('id', $edge['to']);
            $fromX = $fromNode['x'] + $fromNode['width'];
            $fromY = $fromNode['y'] + ($fromNode['height'] / 2);
            $toX = $toNode['x'];
            $toY = $toNode['y'] + ($toNode['height'] / 2);
        @endphp
        <x-edge :from="['x' => $fromX, 'y' => $fromY]" :to="['x' => $toX, 'y' => $toY]" />
    @endforeach
</div>
