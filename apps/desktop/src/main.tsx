import "./index.css"
import ReactDOM from "react-dom/client"
import App from "./App"
import { ConvexProvider, ConvexReactClient } from "convex/react"
import { AuthProvider } from "@/contexts/AuthContext"

// Type assertion for ConvexProvider to fix TypeScript strict JSX checking
const ConvexProviderComponent = ConvexProvider as any;

const convexUrl = import.meta.env.VITE_CONVEX_URL;
if (!convexUrl) {
  throw new Error("VITE_CONVEX_URL environment variable is required");
}
const convex = new ConvexReactClient(convexUrl);

ReactDOM.createRoot(document.getElementById("root") as HTMLElement).render(
  <AuthProvider>
    <ConvexProviderComponent client={convex}>
      <App />
    </ConvexProviderComponent>
  </AuthProvider>,
);
