<div x-data="{
    fromX: {{ $from['x'] }},
    fromY: {{ $from['y'] }},
    toX: {{ $to['x'] }},
    toY: {{ $to['y'] }},
    svg: null
}"
x-init="svg = $refs.svg"
@node-moved.window="if ($event.detail.nodeId === {{ $from['id'] }}) {
    let svgPoint = svg.createSVGPoint();
    svgPoint.x = $event.detail.newX;
    svgPoint.y = $event.detail.newY;
    svgPoint = svgPoint.matrixTransform(svg.getScreenCTM().inverse());
    fromX = svgPoint.x;
    fromY = svgPoint.y;
    console.log('Transformed FROM coordinates:', fromX, fromY);
} else if ($event.detail.nodeId === {{ $to['id'] }}) {
    let svgPoint = svg.createSVGPoint();
    svgPoint.x = $event.detail.newX;
    svgPoint.y = $event.detail.newY;
    svgPoint = svgPoint.matrixTransform(svg.getScreenCTM().inverse());
    toX = svgPoint.x;
    toY = svgPoint.y;
    console.log('Transformed TO coordinates:', toX, toY);
}">
    <svg x-ref="svg" style="position: absolute; top: 0; left: 0; width: 100%; height: 100%;">
        <line :x1="fromX" :y1="fromY"
              :x2="toX" :y2="toY" stroke="white"
              stroke-width="2" />
    </svg>
</div>
