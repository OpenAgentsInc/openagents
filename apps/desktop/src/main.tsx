import "./index.css"
import ReactDOM from "react-dom/client"
import App from "./App"
import { ConvexProvider, ConvexReactClient } from "convex/react"
import { AuthProvider } from "@/contexts/AuthContext"

const convexUrl = import.meta.env.VITE_CONVEX_URL;
const convex = new ConvexReactClient(convexUrl);

ReactDOM.createRoot(document.getElementById("root") as HTMLElement).render(
  <AuthProvider>
    <ConvexProvider client={convex}>
      <App />
    </ConvexProvider>
  </AuthProvider>,
);
