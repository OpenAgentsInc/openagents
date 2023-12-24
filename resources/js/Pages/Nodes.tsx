import { DraggableList } from "@/Components/nodes/DraggableList";
import { Card, CardContent, CardDescription, CardHeader, CardTitle } from "@/Components/ui/card";

export default function Nodes() {
  const items = [
    <Card>
      <CardHeader>
        <CardTitle>Card Title</CardTitle>
      </CardHeader>
      <CardDescription>
        Lorem ipsum dolor sit
      </CardDescription>
      <CardContent>
        Lorem ipsum dolor sit amet consectetur adipisicing elit. Quisquam, voluptatum.
      </CardContent>
    </Card>,
    <Card>
      <CardHeader>
        <CardTitle>Card Title</CardTitle>
      </CardHeader>
      <CardDescription>
        Lorem ipsum dolor sit
      </CardDescription>
      <CardContent>
        Lorem ipsum dolor sit amet consectetur adipisicing elit. Quisquam, voluptatum.
      </CardContent>
    </Card>,
  ]

  return (
    <div className="flex flex-col justify-center items-center h-screen">
      <DraggableList items={items} />
    </div>
  )
}
