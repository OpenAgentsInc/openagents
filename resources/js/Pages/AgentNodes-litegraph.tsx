import { SidebarLayout } from "@/Layouts/SidebarLayout"
import { usePage } from "@inertiajs/react"
import { Card, CardContent, CardDescription, CardHeader, CardTitle } from '@/Components/ui/card';
import { LGraph, LGraphCanvas, LiteGraph } from 'litegraph.js'
import 'litegraph.js/css/litegraph.css'
import { useEffect } from 'react'

// We show all relevant agent nodes, starting with steps.

interface Agent {
  tasks: Task[]
}

interface Task {
  description: string
  steps: Step[]
}

interface Step {
  agent_id: number
  category: string
  created_at: string
  description: string
  entry_type: string
  error_message: string
  id: number
  name: string
  order: number
  params: any
  success_action: string
  task_id: number
  updated_at: string
}

function AgentNodes() {
  const props = usePage().props as any
  const agent = props.agent as Agent
  const task = agent.tasks[0] as Task
  const steps = task.steps as Step[]

  useEffect(() => {
    var graph = new LGraph();

    var canvas = new LGraphCanvas("#mycanvas", graph, { autoresize: true });
    canvas.resize()

    let prevNode
    steps.forEach((step, index) => {
      // Using a general-purpose node type and setting the title
      var node = LiteGraph.createNode("basic/data");
      node.title = step.name;
      node.pos = [200 + index * 100, 200 + index * 100];

      graph.add(node);

      if (step.entry_type === 'input') {
        node.addInput("userInput", "string")
      } else if (step.entry_type === 'node') {
        // Connect this node to the previous node
        // let prevNode = graph.getNodeById(step.id - 1)
        if (prevNode) {
          node.connect(0, prevNode, 0);
        } else {
          console.log('No previous node found')
        }
      }

      // Customize the node as needed
      node.setValue(step.description);
      prevNode = node
    });

    graph.start();
  }, [])

  return (
    <canvas id="mycanvas" className="w-screen h-screen" />
  )
}

AgentNodes.layout = (page) => <SidebarLayout children={page} grid={true} />

export default AgentNodes
