import "./index.css"
import ReactDOM from "react-dom/client"
import App from "./App"
import { AuthProvider } from "@/contexts/AuthContext"
import { ConvexProviderWithAuth } from "@/components/providers/ConvexProviderWithAuth"

ReactDOM.createRoot(document.getElementById("root") as HTMLElement).render(
  <AuthProvider>
    <ConvexProviderWithAuth>
      <App />
    </ConvexProviderWithAuth>
  </AuthProvider>,
);
