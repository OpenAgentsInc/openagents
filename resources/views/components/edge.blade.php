@props(['from', 'to'])

    <!-- Edge SVG line -->
    <line x1="{{ $from['x'] }}" y1="{{ $from['y'] }}"
        x2="{{ $to['x'] }}" y2="{{ $to['y'] }}" stroke="white"
        stroke-width="2" />
