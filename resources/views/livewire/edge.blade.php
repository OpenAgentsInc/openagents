<div x-data="{
        fromX: {{ $from['x'] }},
        fromY: {{ $from['y'] }},
        toX: {{ $to['x'] }},
        toY: {{ $to['y'] }},
        svg: null
    }"
    x-init="svg = $refs.svg"
    @node-moved.window="
        function(event) {
            if (event.detail.nodeId === {{ $from['id'] }}) {
                let svgPoint = svg.createSVGPoint();
                svgPoint.x = event.detail.startX + event.detail.deltaX;
                svgPoint.y = event.detail.startY + event.detail.deltaY;
                svgPoint = svgPoint.matrixTransform(svg.getScreenCTM().inverse());
                fromX = svgPoint.x;
                fromY = svgPoint.y;
            } else if (event.detail.nodeId === {{ $to['id'] }}) {
                let svgPoint = svg.createSVGPoint();
                svgPoint.x = event.detail.startX + event.detail.deltaX;
                svgPoint.y = event.detail.startY + event.detail.deltaY;
                svgPoint = svgPoint.matrixTransform(svg.getScreenCTM().inverse());
                toX = svgPoint.x;
                toY = svgPoint.y;
            }
            console.log('Edge updated for node ' + event.detail.nodeId + ': from (' + fromX + ', ' + fromY + ') to (' + toX + ', ' + toY + ')');
        }">
    <svg x-ref="svg" style="position: absolute; top: 0; left: 0; width: 100%; height: 100%;">
        <line :x1="fromX" :y1="fromY" :x2="toX" :y2="toY" stroke="white" stroke-width="2" />
    </svg>
</div>
