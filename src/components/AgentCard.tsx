// ... existing code ...
return (
  <div className="relative">
    <div className="absolute w-full z-10">
      <div className="bg-white rounded-lg shadow-lg p-4 border border-gray-200">
        <div className="flex items-center space-x-4">
          <div className="flex-shrink-0">
            <img
              src={agent.avatar || '/default-avatar.png'}
              alt={agent.name}
              className="w-12 h-12 rounded-full"
            />
          </div>
          <div className="flex-1 min-w-0">
            <h3 className="text-sm font-medium text-gray-900 truncate">
              {agent.name}
            </h3>
            <p className="text-sm text-gray-500 truncate">
              {agent.description}
            </p>
          </div>
        </div>
      </div>
    </div>
  </div>
);
// ... existing code ...
