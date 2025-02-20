import { GitBranch, Github, Plus, Trash2 } from "lucide-react"
import { memo, useState } from "react"
import { Button } from "~/components/ui/button"
import {
  Popover, PopoverContent, PopoverTrigger
} from "~/components/ui/popover"
import { cn } from "~/lib/utils"

interface Repo {
  owner: string;
  name: string;
  branch: string;
}

interface RepoFormProps {
  repo: {
    owner: string;
    name: string;
    branch: string;
  };
  isEditing: boolean;
  onSubmit: (e: React.FormEvent) => void;
  onChange: (e: React.ChangeEvent<HTMLInputElement>) => void;
}

const RepoForm = memo(function RepoForm({
  repo,
  isEditing,
  onSubmit,
  onChange,
}: RepoFormProps) {
  return (
    <form onSubmit={onSubmit} className="space-y-2">
      <input
        type="text"
        name="owner"
        value={repo.owner}
        onChange={onChange}
        placeholder="Owner"
        className={cn(
          "w-full p-2 border  text-sm",
          "bg-background dark:bg-background",
          "text-foreground dark:text-foreground",
          "border-input dark:border-input",
          "focus:outline-none focus:border-ring dark:focus:border-ring",
          "placeholder:text-muted-foreground dark:placeholder:text-muted-foreground",
        )}
        autoComplete="off"
      />
      <input
        type="text"
        name="name"
        value={repo.name}
        onChange={onChange}
        placeholder="Repo name"
        className={cn(
          "w-full p-2 border  text-sm",
          "bg-background dark:bg-background",
          "text-foreground dark:text-foreground",
          "border-input dark:border-input",
          "focus:outline-none focus:border-ring dark:focus:border-ring",
          "placeholder:text-muted-foreground dark:placeholder:text-muted-foreground",
        )}
        autoComplete="off"
      />
      <input
        type="text"
        name="branch"
        value={repo.branch}
        onChange={onChange}
        placeholder="Branch"
        className={cn(
          "w-full p-2 border  text-sm",
          "bg-background dark:bg-background",
          "text-foreground dark:text-foreground",
          "border-input dark:border-input",
          "focus:outline-none focus:border-ring dark:focus:border-ring",
          "placeholder:text-muted-foreground dark:placeholder:text-muted-foreground",
        )}
        autoComplete="off"
      />
      <Button
        type="submit"
        className="w-full"
        disabled={!repo.owner || !repo.name}
      >
        {isEditing ? "Update Repository" : "Add Repository"}
      </Button>
    </form>
  );
});

interface RepoSelectorProps {
  selectedRepos: Repo[];
  onReposChange: (repos: Repo[]) => void;
  className?: string;
  showOnlyAddButton?: boolean;
}

export function RepoSelector({
  selectedRepos,
  onReposChange,
  className,
  showOnlyAddButton,
}: RepoSelectorProps) {
  const [open, setOpen] = useState<string | null>(null);
  const [editingRepo, setEditingRepo] = useState<Repo | null>(null);
  const [repoInput, setRepoInput] = useState({
    owner: "",
    name: "",
    branch: "",
  });

  const handleRepoInputChange = (e: React.ChangeEvent<HTMLInputElement>) => {
    setRepoInput({ ...repoInput, [e.target.name]: e.target.value });
  };

  const handleRepoSubmit = (e: React.FormEvent) => {
    e.preventDefault();
    if (editingRepo) {
      onReposChange(
        selectedRepos.map((repo) => (repo === editingRepo ? repoInput : repo)),
      );
      setEditingRepo(null);
    } else {
      onReposChange([...selectedRepos, repoInput]);
    }
    setRepoInput({ owner: "", name: "", branch: "" });
    setOpen(null);
  };

  const handleRemoveRepo = (repoToRemove: Repo) => {
    onReposChange(selectedRepos.filter((repo) => repo !== repoToRemove));
  };

  const handleEditClick = (repo: Repo) => {
    setEditingRepo(repo);
    setRepoInput(repo);
    setOpen(`${repo.owner}/${repo.name}/${repo.branch}`);
  };

  return (
    <div className={cn("flex items-center gap-1.5", className)}>
      <div className="flex gap-1.5 overflow-x-auto hide-scrollbar">
        {selectedRepos.map((repo) => {
          const key = `${repo.owner}/${repo.name}/${repo.branch}`;
          return (
            <Popover
              key={key}
              open={open === key}
              onOpenChange={(isOpen) => setOpen(isOpen ? key : null)}
            >
              <PopoverTrigger asChild>
                <div
                  role="button"
                  tabIndex={0}
                  className={cn(
                    "border-input ring-ring/10 dark:ring-ring/20",
                    "h-9 px-3.5 py-2 border bg-transparent",
                    "text-primary hover:bg-accent hover:text-accent-foreground",
                    "shadow-xs transition-[color,box-shadow]",
                    "focus-visible:ring-4 focus-visible:outline-1",
                    "flex items-center gap-2 text-xs cursor-pointer shrink-0",
                  )}
                  onClick={() => handleEditClick(repo)}
                >
                  <Github className="w-3 h-3" />
                  <span>
                    {repo.owner}/{repo.name}
                  </span>
                  <span className="flex items-center text-xs opacity-80">
                    <GitBranch className="w-3 h-3 mr-1" />
                    {repo.branch}
                  </span>
                  <button
                    onClick={(e) => {
                      e.stopPropagation();
                      handleRemoveRepo(repo);
                    }}
                    className="ml-2 hover:text-destructive"
                  >
                    <Trash2 className="w-3 h-3" />
                  </button>
                </div>
              </PopoverTrigger>
              <PopoverContent sideOffset={4}>
                <RepoForm
                  repo={repoInput}
                  isEditing={!!editingRepo}
                  onSubmit={handleRepoSubmit}
                  onChange={handleRepoInputChange}
                />
              </PopoverContent>
            </Popover>
          );
        })}
      </div>
      {!showOnlyAddButton && (
        <Popover
          open={open === "new"}
          onOpenChange={(isOpen) => {
            setOpen(isOpen ? "new" : null);
            if (isOpen) {
              setEditingRepo(null);
              setRepoInput({ owner: "", name: "", branch: "" });
            }
          }}
        >
          <PopoverTrigger asChild>
            <Button
              type="button"
              data-repo-add-button="true"
              className={cn(
                "border-input ring-ring/10 dark:ring-ring/20",
                "h-9 px-3.5 py-2 border bg-transparent",
                "text-primary hover:bg-accent hover:text-accent-foreground",
                "shadow-xs transition-[color,box-shadow]",
                "focus-visible:ring-4 focus-visible:outline-1",
                "!-none shrink-0",
              )}
            >
              <Github className="w-4 h-4" />
            </Button>
          </PopoverTrigger>
          <PopoverContent sideOffset={4}>
            <RepoForm
              repo={repoInput}
              isEditing={!!editingRepo}
              onSubmit={handleRepoSubmit}
              onChange={handleRepoInputChange}
            />
          </PopoverContent>
        </Popover>
      )}
    </div>
  );
}
