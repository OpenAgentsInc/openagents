<!doctype html>
<html lang="en-us">
<head>
    <meta charset="utf-8">
    <title>Emscripten-Generated Code</title>
    <style>
        canvas.emscripten {
            border: 0px none;
            background-color: black;
        }
    </style>
    @stack('scripts')
    @include('partials.vite')
    @include('partials.analytics')
</head>
<body>

<canvas class="emscripten h-screen w-screen" id="canvas" oncontextmenu="event.preventDefault()"
        tabindex=-1></canvas>

<script type='text/javascript'>
    var Module = {
        canvas: (() => {
            var canvas = document.getElementById('canvas');
            return canvas;
        })(),
    };
</script>
<script async type="text/javascript" src="hi.js"></script>
</body>
</html>
