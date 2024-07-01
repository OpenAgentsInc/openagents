import React, { useState } from "react";
import { useCodebaseStore } from "../../store";

export function Codebases() {
  const { codebases, toggleCodebase, addCodebase, removeCodebase } =
    useCodebaseStore();
  const [newCodebaseName, setNewCodebaseName] = useState("");
  const [newCodebaseBranch, setNewCodebaseBranch] = useState("");

  const handleAddCodebase = (e: React.FormEvent) => {
    e.preventDefault();
    if (newCodebaseName && newCodebaseBranch) {
      addCodebase(newCodebaseName, newCodebaseBranch);
      setNewCodebaseName("");
      setNewCodebaseBranch("");
    }
  };

  return (
    <div className="text-white">
      <h2 className="text-2xl font-bold mb-4">Codebases</h2>
      <ul>
        {codebases.map((codebase) => (
          <li key={codebase.name} className="mb-2 flex items-center">
            <label className="flex items-center flex-grow">
              <input
                type="checkbox"
                checked={codebase.isSelected}
                onChange={() => toggleCodebase(codebase.name)}
                className="mr-2"
              />
              <span>
                {codebase.name} ({codebase.branch})
              </span>
            </label>
            <button
              onClick={() => removeCodebase(codebase.name)}
              className="ml-2 text-red-500 hover:text-red-700"
            >
              Remove
            </button>
          </li>
        ))}
      </ul>
      <form onSubmit={handleAddCodebase} className="mt-4">
        <input
          type="text"
          value={newCodebaseName}
          onChange={(e) => setNewCodebaseName(e.target.value)}
          placeholder="Codebase name"
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
        >
          Add Codebase
        </button>
      </form>
    </div>
  );
}
