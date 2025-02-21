import { useState } from "react";

export interface RepoSelectorProps {
  repos?: string[];
  onAdd?: (repo: string) => void;
  onRemove?: (index: number) => void;
}

export function RepoSelector({ repos = [], onAdd, onRemove }: RepoSelectorProps) {
  const [newRepo, setNewRepo] = useState("");

  const handleSubmit = (e: React.FormEvent) => {
    e.preventDefault();
    if (newRepo.trim() && onAdd) {
      onAdd(newRepo.trim());
      setNewRepo("");
    }
  };

  return (
    <div className="flex flex-wrap gap-2">
      {repos.map((repo, index) => (
        <div
          key={index}
          className="flex items-center gap-1 bg-muted px-2 py-1 rounded-md text-sm"
        >
          <span>{repo}</span>
          {onRemove && (
            <button
              type="button"
              onClick={() => onRemove(index)}
              className="text-muted-foreground hover:text-foreground"
            >
              ×
            </button>
          )}
        </div>
      ))}
      <form onSubmit={handleSubmit} className="flex gap-2">
        <input
          type="text"
          value={newRepo}
          onChange={(e) => setNewRepo(e.target.value)}
          placeholder="owner/repo#branch"
          className="flex-1 px-2 py-1 text-sm bg-transparent border rounded-md focus:outline-none focus:ring-1"
        />
        {onAdd && (
          <button
            type="submit"
            disabled={!newRepo.trim()}
            className="px-2 py-1 text-sm bg-primary text-primary-foreground rounded-md disabled:opacity-50"
          >
            Add
          </button>
        )}
      </form>
    </div>
  );
}