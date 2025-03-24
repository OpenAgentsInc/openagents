import { useState } from "react";
import reactLogo from "./assets/react.svg";
import { invoke } from "@tauri-apps/api/core";
import { Button } from "@openagents/ui";
import "./App.css";

function App() {
  const [greetMsg, setGreetMsg] = useState("");
  const [name, setName] = useState("");
  const [loading, setLoading] = useState(false);

  async function greet() {
    // Learn more about Tauri commands at https://tauri.app/develop/calling-rust/
    setLoading(true);
    try {
      setGreetMsg(await invoke("greet", { name }));
    } finally {
      setLoading(false);
    }
  }

  return (
    <main className="container">
      <h1>Welcome to Tauri + React</h1>

      <div className="row">
        <a href="https://vitejs.dev" target="_blank">
          <img src="/vite.svg" className="logo vite" alt="Vite logo" />
        </a>
        <a href="https://tauri.app" target="_blank">
          <img src="/tauri.svg" className="logo tauri" alt="Tauri logo" />
        </a>
        <a href="https://reactjs.org" target="_blank">
          <img src={reactLogo} className="logo react" alt="React logo" />
        </a>
      </div>
      <p>Click on the Tauri, Vite, and React logos to learn more.</p>

      <form
        className="row"
        onSubmit={(e) => {
          e.preventDefault();
          greet();
        }}
      >
        <input
          id="greet-input"
          onChange={(e) => setName(e.currentTarget.value)}
          placeholder="Enter a name..."
        />
        
        {/* Example of using our shared Button component */}
        <div style={{ marginTop: 10, display: 'flex', gap: 10 }}>
          <Button 
            label="Primary Button" 
            variant="primary" 
            loading={loading} 
            onPress={greet} 
          />
          
          <Button 
            label="Secondary" 
            variant="secondary" 
            onPress={() => alert('Secondary button clicked')} 
          />
          
          <Button 
            label="Tertiary" 
            variant="tertiary" 
            onPress={() => alert('Tertiary button clicked')} 
          />
          
          <Button 
            label="Disabled" 
            disabled 
            onPress={() => alert('This should not show')} 
          />
        </div>
      </form>
      <p>{greetMsg}</p>
    </main>
  );
}

export default App;
