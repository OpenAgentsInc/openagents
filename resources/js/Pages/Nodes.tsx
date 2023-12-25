import { DraggableList } from "@/Components/nodes/DraggableList";
import { Card, CardContent, CardDescription, CardHeader, CardTitle } from "@/Components/ui/card";
import { SidebarLayout } from "@/Layouts/SidebarLayout";

function Nodes() {
  const cards = [{
    title: 'Agent Details',
    description: 'Metadata',
    content: 'Owner, Type (autodev | concierge), Template, Created, Name, Description'
  },
  {
    title: 'LLM chat API call',
    description: 'Pass user input to chat model',
    content: '"Who am I talking to?" sent to DiscoLM-mixtral-8x7b-v2 via Together API'
  }]

  const items = cards.map((card, index) => {
    return (
      <Card key={index}>
        <CardHeader>
          <CardTitle>{card.title}</CardTitle>
          <CardDescription>
            {card.description}
          </CardDescription>
        </CardHeader>
        <CardContent>
          {card.content}
        </CardContent>
      </Card>
    )
  })

  return (
    <div className="flex flex-col justify-center items-center h-screen">
      <DraggableList items={items} />
    </div>
  )
}

Nodes.layout = (page) => <SidebarLayout children={page} />

export default Nodes
