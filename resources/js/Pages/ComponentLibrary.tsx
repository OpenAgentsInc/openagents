import { Button } from "@/Components/ui/button"
import { Head } from "@inertiajs/react"

const variants = ["default", "destructive", "outline", "secondary", "ghost", "link"] as const
const sizes = ["default", "sm", "lg", "icon"] as const

export default function ComponentLibrary() {
  return (
    <>
      <Head title="Component Library" />
      <div className='dark fixed font-mono bg-black text-white h-screen w-screen p-8'>
        <h1 className="text-3xl font-bold mb-8">Component Library</h1>
        <h2 className="text-2xl font-semibold mb-4">Buttons</h2>
        <div className="grid grid-cols-4 gap-4">
          {variants.map((variant) => (
            <div key={variant} className="space-y-4">
              <h3 className="text-xl font-medium capitalize">{variant}</h3>
              {sizes.map((size) => (
                <div key={size} className="flex items-center space-x-2">
                  <Button variant={variant} size={size}>
                    {size === "icon" ? "+" : `${variant} ${size}`}
                  </Button>
                  <span className="text-sm">{size}</span>
                </div>
              ))}
            </div>
          ))}
        </div>
      </div>
    </>
  );
}
