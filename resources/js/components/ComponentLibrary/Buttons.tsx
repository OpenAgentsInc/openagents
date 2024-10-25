import { Plus } from "lucide-react"
import React from "react"
import { Button } from "@/components/ui/button"

const variants = ["default", "destructive", "outline", "secondary", "ghost", "link"] as const
const sizes = ["default", "sm", "lg", "icon"] as const

export function Buttons() {
  return (
    <div className="space-y-8">
      <h2 className="text-2xl font-semibold mb-4">Buttons</h2>
      <div className="grid grid-cols-1 md:grid-cols-2 lg:grid-cols-3 xl:grid-cols-4 gap-8">
        {variants.map((variant) => (
          <div key={variant} className="space-y-4">
            <h3 className="text-xl font-medium capitalize">{variant}</h3>
            {sizes.map((size) => (
              <div key={size} className="flex items-center space-x-2">
                <Button variant={variant} size={size}>
                  {size === "icon" ? <Plus className="h-4 w-4" /> : `${variant} ${size}`}
                </Button>
                <span className="text-sm">{size}</span>
              </div>
            ))}
          </div>
        ))}
      </div>
    </div>
  );
}
