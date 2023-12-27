import { LGraph, LGraphCanvas, LiteGraph } from 'litegraph.js'
import 'litegraph.js/css/litegraph.css'
import { useEffect } from 'react'

export default function Graph() {
  useEffect(() => {
    var graph = new LGraph();

    var canvas = new LGraphCanvas("#mycanvas", graph, { autoresize: true });
    canvas.resize()

    var node_const = LiteGraph.createNode("basic/const");
    node_const.pos = [120, 200];
    graph.add(node_const);
    node_const.setValue(4.5);

    var node_watch = LiteGraph.createNode("basic/watch");
    node_watch.pos = [700, 200];
    graph.add(node_watch);

    node_const.connect(0, node_watch, 0);

    graph.start()
  }, [])
  return (
    <canvas id="mycanvas" className="w-screen h-screen" />
  )
}
