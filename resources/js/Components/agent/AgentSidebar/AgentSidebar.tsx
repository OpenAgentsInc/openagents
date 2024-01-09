import { IconBrain, IconGraph } from "../AgentIcons";

export const AgentSidebar = ({ agent, children }) => {
  return (
    <div className="h-full w-full md:h-screen">
      <div className="flex h-full bg-neutral-50">
        <div className="space-y-4 hidden w-22 flex-col items-center border-r border-neutral-300 p-3 pt-5 lg:flex">
          <div className="cursor-pointer mb-1 flex h-16 w-16 flex-col items-center justify-center rounded-xl text-neutral-900 hover:bg-neutral-200 hover:text-neutral-900">
            <IconBrain className="h-12 w-12" />
            <div className="t-label">Knowledge</div>
          </div>
          <div className="cursor-pointer mb-1 flex h-16 w-16 flex-col items-center justify-center rounded-xl text-neutral-900 hover:bg-neutral-200 hover:text-neutral-900">
            <IconGraph />
            <div className="t-label mt-2">Graph</div>
          </div>
        </div>
        <div className="relative grow overflow-x-auto flex flex-col">
          {children}
        </div>
      </div>
    </div >
  )
}
