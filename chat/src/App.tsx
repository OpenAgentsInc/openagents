import { BrowserRouter, Navigate, Route, Routes } from "react-router-dom"
import ChatScreen from "./pages/ChatScreen"
import LoginScreen from "./pages/LoginScreen"

function App() {
  return (
    <BrowserRouter basename="/chat">
      <Routes>
        <Route path="/" element={<LoginScreen />} />
        <Route path="/new" element={<ChatScreen />} />
        <Route path="*" element={<Navigate to="/" replace />} />
      </Routes>
    </BrowserRouter>
  );
}

export default App;
