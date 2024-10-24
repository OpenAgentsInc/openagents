import { Button } from "@/Components/ui/button"
import { Head } from "@inertiajs/react"

export default function ComponentLibrary() {
  return (
    <>
      <Head title="Component Library" />
      <div className='font-mono bg-black text-white h-screen w-screen'>
        <h2>Component Library</h2>
        <Button>nice</Button>
      </div>
    </>
  );
}
