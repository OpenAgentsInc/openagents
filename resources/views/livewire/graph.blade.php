<div>

    <div class="my-12 w-full h-[500px] border-2 border-offblack bg-black relative rounded-xl">

        <!-- Background grid of dots -->
        <svg style="position: absolute; top: 0; left: 0; width: 100%; height: 100%;">
            <defs>
                <pattern id="grid" width="20" height="20" patternUnits="userSpaceOnUse">
                    <circle cx="10" cy="10" r="0.5" fill="rgba(255,255,255,0.7)" />
                </pattern>
            </defs>
            <rect width="100%" height="100%" fill="url(#grid)" />
        </svg>

        @foreach($nodes as $node)
            <livewire:node :id="$node['id']" :x="$node['x']" :y="$node['y']" :title="$node['title']"
                :width="$node['width']" :height="$node['height']" :key="'node-'.$node['id']">
        @endforeach

        @foreach($edges as $edge)
            <livewire:edge :from="$edge['from']" :to="$edge['to']" :nodes="$nodes"
                :key="'edge-'.$edge['from'].'-'.$edge['to']">
        @endforeach
    </div>
</div>
