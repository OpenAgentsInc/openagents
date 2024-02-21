<div id="node-{{ $id }}" class="node z-[9999] touch-none"
     style="position: absolute; left: {{ $x }}px; top: {{ $y }}px;"
     data-node-id="{{ $id }}"
     x-data="{
         dragging: false,
         x: {{ $x }},
         y: {{ $y }},
         offsetX: 0,
         offsetY: 0
     }"
     @mousedown="function(event) {
         dragging = true;
         offsetX = event.clientX - this.x;
         offsetY = event.clientY - this.y;
     }"
     @mousemove.window="function(event) {
         if (dragging) {
             let newX = event.clientX - offsetX;
             let newY = event.clientY - offsetY;
             this.x = newX;
             this.y = newY;
             $dispatch('node-moved', {
                 nodeId: {{ $id }},
                 x: newX,
                 y: newY,
                 width: {{ $width }},
                 height: {{ $height }}
             });
             console.log('Firing node-moved event with x: ' + newX + ', y: ' + newY);
         }
     }"
     @mouseup.window="dragging = false"
     @mouseleave.window="if(dragging){ dragging = false; }"
>
    <svg width="{{ $width }}" height="{{ $height }}" xmlns="http://www.w3.org/2000/svg">
        <!-- White rectangle with 1px white border -->
        <rect x="{{ $strokeWidth + $circleOffset }}" y="{{ $strokeWidth }}"
            width="{{ $width - 2 * $circleOffset }}" height="{{ $height - 2 * $strokeWidth }}"
            fill="black" stroke="white" stroke-width="{{ $strokeWidth }}" />

        <!-- Hollow circles with white stroke on the left and right edges, centered on the line -->
        <circle cx="{{ $circleOffset }}" cy="{{ $height / 2 }}" r="{{ $radius }}" fill="black" stroke="white"
            stroke-width="{{ $circleStrokeWidth }}" />
        <circle cx="{{ $width - $circleOffset }}" cy="{{ $height / 2 }}" r="{{ $radius }}" fill="black"
            stroke="white" stroke-width="{{ $circleStrokeWidth }}" />

        <!-- Node title text, centered -->
        <text class="pointer-events-none" x="{{ $width / 2 }}" y="{{ $height / 2 }}" font-family="JetBrains Mono" font-size="18"
            fill="white" text-anchor="middle" dominant-baseline="middle">{{ $title }}</text>
    </svg>
</div>
