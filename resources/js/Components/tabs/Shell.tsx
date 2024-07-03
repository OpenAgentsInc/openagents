import React, { useEffect, useRef } from "react";
import { Terminal } from "xterm";
import { FitAddon } from "xterm-addon-fit";
import "xterm/css/xterm.css";

export function Shell() {
  const terminalRef = useRef(null);
  const terminalInstanceRef = useRef(null);
  const socketRef = useRef(null);

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
    socketRef.current = socket;

    socket.onopen = () => {
      console.log("WebSocket connection established");
      term.writeln("Connected to EC2 instance");
    };

    socket.onmessage = (event) => {
      const data = JSON.parse(event.data);
      console.log("Received message:", data);

      switch (data.type) {
        case "connection":
          term.writeln(data.content);
          break;
        case "shell_command_result":
          term.writeln("Shell command result:");
          term.writeln(data.content);
          break;
        default:
          console.log("Unknown message type:", data.type);
          term.writeln(`Received: ${data.content}`);
      }
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

      // Handle user input
      term.onData((data) => {
        if (data === "\r") {
          // Enter key
          const command = term.buffer.active
            .getLine(term.buffer.active.cursorY)
            .translateToString();
          sendCommand(command.trim());
          term.write("\r\n");
        } else {
          term.write(data);
        }
      });
    }

    // Clean up on unmount
    return () => {
      if (terminalInstanceRef.current) {
        terminalInstanceRef.current.dispose();
      }
      if (socketRef.current) {
        socketRef.current.close();
      }
    };
  }, []);

  const sendCommand = (command) => {
    if (socketRef.current && socketRef.current.readyState === WebSocket.OPEN) {
      socketRef.current.send(
        JSON.stringify({
          type: "shell_command",
          content: command,
        }),
      );
    } else {
      console.error("WebSocket is not connected");
    }
  };

  return <div ref={terminalRef} className="w-full h-full" />;
}
