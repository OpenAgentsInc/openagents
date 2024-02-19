<div x-on:updateNodePosition="console.log('HMHMM')">
    <svg style="position: absolute; top: 0; left: 0; width: 100%; height: 100%;">
        <line x1="{{ $from['x'] }}" y1="{{ $from['y'] }}"
            x2="{{ $to['x'] }}" y2="{{ $to['y'] }}" stroke="white"
            stroke-width="2" />
    </svg>
</div>
