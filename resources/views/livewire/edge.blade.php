<div x-data="{
        fromX: {{ $from['x'] }},
        fromY: {{ $from['y'] }},
        toX: {{ $to['x'] }},
        toY: {{ $to['y'] }},
        svg: null
    }"
    x-init="$nextTick(() => { svg = $refs.svg })"
    @node-moved.window="
        function(event) {
            if (!svg) return;
            let svgPoint = svg.createSVGPoint();
            svgPoint.x = event.detail.x;
            svgPoint.y = event.detail.y;
            if (event.detail.nodeId === {{ $from['id'] }}) {
                fromX = svgPoint.x + event.detail.width - 2; // minus the circle offset but we can hardcode it for now
                fromY = svgPoint.y + event.detail.height / 2; // Center vertically
            } else if (event.detail.nodeId === {{ $to['id'] }}) {
                toX = svgPoint.x + 2; // Adjust for the left side
                toY = svgPoint.y + event.detail.height / 2; // Center vertically
            }
        }">
    <svg x-ref="svg" style="position: absolute; top: 0; left: 0; width: 100%; height: 100%;">
        <line :x1="fromX" :y1="fromY" :x2="toX" :y2="toY" stroke="white" stroke-width="2" />
    </svg>
</div>
