@props(['id', 'x', 'y', 'title'])

<!-- Node SVG component positioned according to x and y props -->
<svg width="400" height="250" style="position: absolute; left: {{ $x }}px; top: {{ $y }}px;" xmlns="http://www.w3.org/2000/svg">
    <!-- White rectangle with 1px white border -->
    <rect x="7" y="1" width="386" height="248" fill="none" stroke="white" stroke-width="1" />
    
    <!-- Hollow circles with white stroke on the left and right edges, filled black -->
    <circle cx="7" cy="125" r="5" fill="black" stroke="white" stroke-width="2" />
    <circle cx="393" cy="125" r="5" fill="black" stroke="white" stroke-width="2" />
    
    <!-- Node title text, centered -->
    <text x="200" y="130" font-family="JetBrains Mono" font-size="20" fill="white" text-anchor="middle" dominant-baseline="central">{{ $title }}</text>
</svg>
