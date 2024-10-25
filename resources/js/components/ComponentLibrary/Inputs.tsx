import React from "react"
import { Input } from "@/components/ui/input"

const inputTypes = ["text", "password", "email", "number", "search", "tel", "url"] as const;

export function Inputs() {
  return (
    <div className="space-y-8">
      <h2 className="text-2xl font-semibold mb-4">Inputs</h2>
      <div className="grid grid-cols-1 md:grid-cols-2 lg:grid-cols-3 xl:grid-cols-4 gap-8">
        {inputTypes.map((type) => (
          <div key={type} className="space-y-4">
            <h3 className="text-xl font-medium capitalize">{type}</h3>
            <div className="space-y-2">
              <Input type={type} placeholder={`Enter ${type}...`} />
              <Input type={type} placeholder={`Disabled ${type}...`} disabled />
            </div>
          </div>
        ))}
      </div>
    </div>
  );
}
