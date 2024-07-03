import React, { useEffect, useRef } from "react";
import { Terminal } from "xterm";
import { FitAddon } from "xterm-addon-fit";
import "xterm/css/xterm.css";

export function Shell() {
  const terminalRef = useRef(null);
  const terminalInstanceRef = useRef(null);

  useEffect(() => {
    // Initialize xterm.js
    const term = new Terminal({
      cursorBlink: true,
      fontSize: 14,
      fontFamily: 'Menlo, Monaco, "Courier New", monospace',
      theme: {
        background: "rgb(24 24 27)",
      },
    });

    const fitAddon = new FitAddon();
    term.loadAddon(fitAddon);

    // Create WebSocket connection
    const socket = new WebSocket("wss://shell.openagents.com/ws");

    socket.onopen = () => {
      console.log("WebSocket connection established");
      term.writeln("Connected to EC2 instance");
    };

    socket.onmessage = (event) => {
      term.write(event.data);
    };

    socket.onerror = (error) => {
      console.error("WebSocket error:", error);
      term.writeln("Error: Unable to connect to EC2 instance");
    };

    socket.onclose = () => {
      console.log("WebSocket connection closed");
      term.writeln("Disconnected from EC2 instance");
    };

    // Render the terminal
    if (terminalRef.current) {
      term.open(terminalRef.current);
      fitAddon.fit();
      terminalInstanceRef.current = term;

      // Adjust terminal size when window resizes
      const resizeObserver = new ResizeObserver(() => {
        fitAddon.fit();
      });
      resizeObserver.observe(terminalRef.current);
    }

    // Clean up on unmount
    return () => {
      if (terminalInstanceRef.current) {
        terminalInstanceRef.current.dispose();
      }
      socket.close();
    };
  }, []);

  return <div ref={terminalRef} className="w-full h-full" />;
}
