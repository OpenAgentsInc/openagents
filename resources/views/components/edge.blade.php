@props(['from', 'to'])

    <!-- Assuming $from and $to are the IDs of the nodes we want to connect -->
    <!-- Edge SVG component -->
    <svg xmlns="http://www.w3.org/2000/svg">
        <!-- Line from one node to another; assuming each node has a width of 400 -->
        <line x1="{{ $from['x'] + 400 }}"
            y1="{{ $from['y'] + 125 }}" x2="{{ $to['x'] }}"
            y2="{{ $to['y'] + 125 }}" stroke="white" stroke-width="2" />
    </svg>
