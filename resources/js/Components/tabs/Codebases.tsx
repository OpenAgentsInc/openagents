import React, { useState, useEffect } from "react";
import { useCodebaseStore } from "../../store";

export function Codebases() {
  const {
    codebases,
    toggleCodebase,
    addCodebase,
    removeCodebase,
    updateCodebaseStatus,
  } = useCodebaseStore();
  const [newCodebaseName, setNewCodebaseName] = useState("");
  const [newCodebaseBranch, setNewCodebaseBranch] = useState("");
  const [isLoading, setIsLoading] = useState(false);

  useEffect(() => {
    // Check status of all codebases on component mount
    codebases.forEach(checkCodebaseStatus);
  }, []);

  const handleAddCodebase = async (e: React.FormEvent) => {
    e.preventDefault();
    if (newCodebaseName && newCodebaseBranch) {
      setIsLoading(true);
      try {
        const response = await fetch("/api/index-repository", {
          method: "POST",
          headers: {
            "Content-Type": "application/json",
          },
          body: JSON.stringify({
            remote: "github",
            repository: newCodebaseName,
            branch: newCodebaseBranch,
          }),
        });

        if (!response.ok) {
          throw new Error("Failed to index repository");
        }

        const result = await response.json();
        addCodebase(newCodebaseName, newCodebaseBranch, result.id);
        setNewCodebaseName("");
        setNewCodebaseBranch("");
      } catch (error) {
        console.error("Error indexing repository:", error);
        alert("Failed to index repository. Please try again.");
      } finally {
        setIsLoading(false);
      }
    }
  };

  const checkCodebaseStatus = async (codebase) => {
    try {
      const response = await fetch(`/api/repository-status/${codebase.id}`);
      if (!response.ok) {
        throw new Error("Failed to fetch repository status");
      }
      const status = await response.json();
      updateCodebaseStatus(codebase.id, status);
    } catch (error) {
      console.error("Error checking repository status:", error);
    }
  };

  return (
    <div className="text-white">
      <h2 className="text-2xl font-bold mb-4">Codebases</h2>
      <ul>
        {codebases.map((codebase) => (
          <li key={codebase.id} className="mb-2 flex items-center">
            <label className="flex items-center flex-grow">
              <input
                type="checkbox"
                checked={codebase.isSelected}
                onChange={() => toggleCodebase(codebase.id)}
                className="mr-2"
              />
              <span>
                {codebase.name} ({codebase.branch}) - Status:{" "}
                {codebase.status || "Unknown"}
              </span>
            </label>
            <button
              onClick={() => removeCodebase(codebase.id)}
              className="ml-2 text-red-500 hover:text-red-700"
            >
              Remove
            </button>
            <button
              onClick={() => checkCodebaseStatus(codebase)}
              className="ml-2 text-blue-500 hover:text-blue-700"
            >
              Check Status
            </button>
          </li>
        ))}
      </ul>
      <form onSubmit={handleAddCodebase} className="mt-4">
        <input
          type="text"
          value={newCodebaseName}
          onChange={(e) => setNewCodebaseName(e.target.value)}
          placeholder="Codebase name (e.g., user/repo)"
          className="mr-2 p-1 text-black"
        />
        <input
          type="text"
          value={newCodebaseBranch}
          onChange={(e) => setNewCodebaseBranch(e.target.value)}
          placeholder="Branch"
          className="mr-2 p-1 text-black"
        />
        <button
          type="submit"
          className="bg-blue-500 hover:bg-blue-700 text-white font-bold py-1 px-2 rounded"
          disabled={isLoading}
        >
          {isLoading ? "Indexing..." : "Add & Index Codebase"}
        </button>
      </form>
    </div>
  );
}
