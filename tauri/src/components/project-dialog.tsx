import { useState } from "react"
import { Folder, FolderOpen } from "lucide-react"
import {
  Dialog,
  DialogContent,
  DialogDescription,
  DialogFooter,
  DialogHeader,
  DialogTitle,
} from "@/components/ui/dialog"
import { Button } from "@/components/ui/button"
import { Input } from "@/components/ui/input"
import { Label } from "@/components/ui/label"
import { Textarea } from "@/components/ui/textarea"
import { pickDirectory } from "@/lib/tauri-acp"
import { createProject } from "@/lib/tauri-projects"
import { useSharedTinyvexWebSocket } from "@/lib/tinyvexWebSocketSingleton"
import { useProjectStore } from "@/lib/project-store"
import { useAssistantRuntime } from "@openagentsinc/assistant-ui-runtime"

interface ProjectDialogProps {
  open: boolean
  onOpenChange: (open: boolean) => void
}

export function ProjectDialog({ open, onOpenChange }: ProjectDialogProps) {
  const [name, setName] = useState("")
  const [path, setPath] = useState("")
  const [description, setDescription] = useState("")
  const [icon, setIcon] = useState<"Folder" | "FolderOpen">("Folder")
  const [color, setColor] = useState("#3b82f6") // default blue
  const [isSubmitting, setIsSubmitting] = useState(false)
  const ws = useSharedTinyvexWebSocket()
  const setActiveProject = useProjectStore((s) => s.setActiveProject)
  const runtime = useAssistantRuntime()

  const handlePickDirectory = async () => {
    try {
      const selectedPath = await pickDirectory()
      if (selectedPath) {
        setPath(selectedPath)
        // Auto-fill name from directory if empty
        if (!name) {
          const dirName = selectedPath.split(/[/\\]/).pop() || ""
          setName(dirName)
        }
      }
    } catch (error) {
      console.error("Failed to pick directory:", error)
    }
  }

  const handleSubmit = async (e: React.FormEvent) => {
    e.preventDefault()

    if (!name.trim() || !path.trim()) {
      return
    }

    setIsSubmitting(true)

    try {
      // Generate a UUID for the project
      const projectId = crypto.randomUUID()

      // Create the project
      if (ws.connected) {
        createProject(ws, {
          id: projectId,
          name: name.trim(),
          path: path.trim(),
          description: description.trim() || null,
          icon,
          color,
          starred: 0,
          archived: 0,
        })
      }

      // Set the newly created project as active
      setActiveProject(projectId)

      // Switch to a new thread so it uses the project's working directory
      if (runtime.switchToNewThread) {
        runtime.switchToNewThread()
      }

      // Close dialog and reset form
      onOpenChange(false)
      setName("")
      setPath("")
      setDescription("")
      setIcon("Folder")
      setColor("#3b82f6")
    } catch (error) {
      console.error("Failed to create project:", error)
    } finally {
      setIsSubmitting(false)
    }
  }

  const handleCancel = () => {
    onOpenChange(false)
    // Reset form
    setName("")
    setPath("")
    setDescription("")
    setIcon("Folder")
    setColor("#3b82f6")
  }

  return (
    <Dialog open={open} onOpenChange={onOpenChange}>
      <DialogContent className="sm:max-w-[500px]">
        <form onSubmit={handleSubmit}>
          <DialogHeader>
            <DialogTitle>Create New Project</DialogTitle>
            <DialogDescription>
              Add a new project to organize your conversations and work.
            </DialogDescription>
          </DialogHeader>

          <div className="grid gap-4 py-4">
            {/* Project Name */}
            <div className="grid gap-2">
              <Label htmlFor="project-name">
                Project Name <span className="text-destructive">*</span>
              </Label>
              <Input
                id="project-name"
                value={name}
                onChange={(e) => setName(e.target.value)}
                placeholder="My Project"
                required
                autoFocus
              />
            </div>

            {/* Project Path */}
            <div className="grid gap-2">
              <Label htmlFor="project-path">
                Directory Path <span className="text-destructive">*</span>
              </Label>
              <div className="flex gap-2">
                <Input
                  id="project-path"
                  value={path}
                  onChange={(e) => setPath(e.target.value)}
                  placeholder="/path/to/project"
                  required
                  className="flex-1"
                />
                <Button
                  type="button"
                  variant="outline"
                  onClick={handlePickDirectory}
                >
                  Browse
                </Button>
              </div>
            </div>

            {/* Description */}
            <div className="grid gap-2">
              <Label htmlFor="project-description">Description</Label>
              <Textarea
                id="project-description"
                value={description}
                onChange={(e) => setDescription(e.target.value)}
                placeholder="Optional description of your project"
                rows={3}
              />
            </div>

            {/* Icon Selection */}
            <div className="grid gap-2">
              <Label>Icon</Label>
              <div className="flex gap-2">
                <Button
                  type="button"
                  variant={icon === "Folder" ? "default" : "outline"}
                  size="sm"
                  onClick={() => setIcon("Folder")}
                  className="flex items-center gap-2"
                >
                  <Folder className="h-4 w-4" />
                  Folder
                </Button>
                <Button
                  type="button"
                  variant={icon === "FolderOpen" ? "default" : "outline"}
                  size="sm"
                  onClick={() => setIcon("FolderOpen")}
                  className="flex items-center gap-2"
                >
                  <FolderOpen className="h-4 w-4" />
                  Open Folder
                </Button>
              </div>
            </div>

            {/* Color Picker */}
            <div className="grid gap-2">
              <Label htmlFor="project-color">Color</Label>
              <div className="flex gap-2 items-center">
                <input
                  type="color"
                  id="project-color"
                  value={color}
                  onChange={(e) => setColor(e.target.value)}
                  onInput={(e) => setColor((e.target as HTMLInputElement).value)}
                  className="h-9 w-16 rounded border cursor-pointer"
                />
                <Input
                  value={color}
                  onChange={(e) => setColor(e.target.value)}
                  placeholder="#3b82f6"
                  className="flex-1 font-mono"
                />
              </div>
            </div>
          </div>

          <DialogFooter>
            <Button
              type="button"
              variant="outline"
              onClick={handleCancel}
              disabled={isSubmitting}
            >
              Cancel
            </Button>
            <Button type="submit" disabled={isSubmitting || !name.trim() || !path.trim()}>
              {isSubmitting ? "Creating..." : "Create Project"}
            </Button>
          </DialogFooter>
        </form>
      </DialogContent>
    </Dialog>
  )
}
