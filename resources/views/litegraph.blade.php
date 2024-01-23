<html>

<head>
    <link rel="stylesheet" type="text/css" href="vendor/litegraph/litegraph.css">
    <script type="text/javascript" src="vendor/litegraph/litegraph.js"></script>
</head>

<body style='width:100%; height:100%'>
    <canvas id='mycanvas' width='1024' height='720' style='border: 1px solid'></canvas>
    <script>
        var graph = new LGraph();

        var canvas = new LGraphCanvas("#mycanvas", graph);

        var node_const = LiteGraph.createNode("basic/const");
        node_const.pos = [200, 200];
        graph.add(node_const);
        node_const.setValue(4.5);

        var node_watch = LiteGraph.createNode("basic/watch");
        node_watch.pos = [700, 200];
        graph.add(node_watch);

        node_const.connect(0, node_watch, 0);

        graph.start()

    </script>
</body>

</html>
