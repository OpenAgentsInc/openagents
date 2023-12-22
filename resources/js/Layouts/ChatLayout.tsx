export default function ChatLayout({ children }) {
  return (
    <div className="bg-gray-100 dark:bg-gray-900 h-screen">
      <div className="flex flex-col h-full">
        <main className="flex-1 overflow-hidden">
          {children}
        </main>
      </div>
    </div>
  )
}
