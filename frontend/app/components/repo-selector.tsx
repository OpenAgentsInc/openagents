import { GitBranch, Github, Plus, Trash2 } from "lucide-react"
import { useState } from "react"
import { Button } from "~/components/ui/button"
import {
  Popover, PopoverContent, PopoverTrigger
} from "~/components/ui/popover"
import {
  Select, SelectContent, SelectItem, SelectTrigger, SelectValue
} from "~/components/ui/select"
import { cn } from "~/lib/utils"

interface Repo {
  owner: string
  name: string
  branch: string
}

interface RepoSelectorProps {
  selectedRepos: Repo[]
  onReposChange: (repos: Repo[]) => void
}

export function RepoSelector({ selectedRepos, onReposChange }: RepoSelectorProps) {
  const [open, setOpen] = useState(false)
  const [editingRepo, setEditingRepo] = useState<Repo | null>(null)
  const [repoInput, setRepoInput] = useState({
    owner: '',
    name: '',
    branch: ''
  })

  const handleRepoInputChange = (e: React.ChangeEvent<HTMLInputElement>) => {
    setRepoInput({ ...repoInput, [e.target.name]: e.target.value })
  }

  const handleRepoSubmit = (e: React.FormEvent) => {
    e.preventDefault()
    if (editingRepo) {
      onReposChange(selectedRepos.map(repo =>
        repo === editingRepo ? repoInput : repo
      ))
      setEditingRepo(null)
    } else {
      onReposChange([...selectedRepos, repoInput])
    }
    setRepoInput({ owner: '', name: '', branch: '' })
    setOpen(false)
  }

  const handleRemoveRepo = (repoToRemove: Repo) => {
    onReposChange(selectedRepos.filter(repo => repo !== repoToRemove))
  }

  return (
    <div className="grow flex gap-1.5">
      <div className="flex flex-wrap gap-1.5 grow">
        {selectedRepos.map((repo) => (
          <div
            key={`${repo.owner}/${repo.name}/${repo.branch}`}
            className={cn(
              "border-input ring-ring/10 dark:ring-ring/20",
              "h-9 px-3.5 py-2 border bg-transparent",
              "text-primary hover:bg-accent hover:text-accent-foreground",
              "shadow-xs transition-[color,box-shadow]",
              "focus-visible:ring-4 focus-visible:outline-1",
              "flex items-center gap-2 text-xs"
            )}
          >
            <Github className="w-3 h-3" />
            <span>{repo.owner}/{repo.name}</span>
            <span className="flex items-center text-xs opacity-80">
              <GitBranch className="w-3 h-3 mr-1" />
              {repo.branch}
            </span>
            <button
              onClick={() => handleRemoveRepo(repo)}
              className="ml-2 hover:text-destructive"
            >
              <Trash2 className="w-3 h-3" />
            </button>
          </div>
        ))}
      </div>
      <Popover open={open} onOpenChange={setOpen}>
        <PopoverTrigger asChild>
          <Button
            type="button"
            className={cn(
              "border-input ring-ring/10 dark:ring-ring/20",
              "h-9 px-3.5 py-2 border bg-transparent",
              "text-primary hover:bg-accent hover:text-accent-foreground",
              "shadow-xs transition-[color,box-shadow]",
              "focus-visible:ring-4 focus-visible:outline-1",
              "!rounded-none"
            )}
          >
            <Plus className="w-4 h-4" />
          </Button>
        </PopoverTrigger>
        <PopoverContent>
          <form onSubmit={handleRepoSubmit} className="space-y-2">
            <input
              type="text"
              name="owner"
              value={repoInput.owner}
              onChange={handleRepoInputChange}
              placeholder="Owner"
              className={cn(
                "w-full p-2 border rounded text-sm",
                "bg-background dark:bg-background",
                "text-foreground dark:text-foreground",
                "border-input dark:border-input",
                "focus:outline-none focus:border-ring dark:focus:border-ring",
                "placeholder:text-muted-foreground dark:placeholder:text-muted-foreground"
              )}
              autoComplete="off"
            />
            <input
              type="text"
              name="name"
              value={repoInput.name}
              onChange={handleRepoInputChange}
              placeholder="Repo name"
              className={cn(
                "w-full p-2 border rounded text-sm",
                "bg-background dark:bg-background",
                "text-foreground dark:text-foreground",
                "border-input dark:border-input",
                "focus:outline-none focus:border-ring dark:focus:border-ring",
                "placeholder:text-muted-foreground dark:placeholder:text-muted-foreground"
              )}
              autoComplete="off"
            />
            <input
              type="text"
              name="branch"
              value={repoInput.branch}
              onChange={handleRepoInputChange}
              placeholder="Branch"
              className={cn(
                "w-full p-2 border rounded text-sm",
                "bg-background dark:bg-background",
                "text-foreground dark:text-foreground",
                "border-input dark:border-input",
                "focus:outline-none focus:border-ring dark:focus:border-ring",
                "placeholder:text-muted-foreground dark:placeholder:text-muted-foreground"
              )}
              autoComplete="off"
            />
            <Button
              type="submit"
              className="w-full"
              disabled={!repoInput.owner || !repoInput.name}
            >
              {editingRepo ? 'Update Repository' : 'Add Repository'}
            </Button>
          </form>
        </PopoverContent>
      </Popover>
    </div>
  )
}
