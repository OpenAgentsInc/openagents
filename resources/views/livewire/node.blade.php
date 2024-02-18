<div id="node-{{ $id }}" class="node z-[9999] touch-none"
    style="position: absolute; left: {{ $x }}px; top: {{ $y }}px;">
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
        <text x="{{ $width / 2 }}" y="{{ $height / 2 }}" font-family="JetBrains Mono" font-size="20"
            fill="white" text-anchor="middle" dominant-baseline="middle">{{ $title }}</text>
    </svg>
</div>
