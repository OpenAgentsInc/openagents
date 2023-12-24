import { DraggableList } from "@/Components/nodes/DraggableList";

export default function Nodes() {
  return (
    <div className="flex flex-col justify-center items-center h-screen">
      <DraggableList items={'Lorem ipsum dolor sit'.split(' ')} />
    </div>
  )
}
