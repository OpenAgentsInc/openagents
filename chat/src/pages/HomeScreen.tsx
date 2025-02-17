import HeaderBar from "@/components/HeaderBar"

export default function HomeScreen() {
  return (
    <div className="dark bg-background text-foreground fixed w-screen min-h-screen">
      <HeaderBar />
      <h1>Home</h1>
    </div>
  );
}
