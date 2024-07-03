import React, { useEffect, useRef, useCallback } from "react";
import { Terminal } from "xterm";
import { FitAddon } from "xterm-addon-fit";
import "xterm/css/xterm.css";
import { useMessageStore } from "../../store";

export function Shell() {
  const terminalRef = useRef(null);
  const terminalInstanceRef = useRef(null);
  const socketRef = useRef(null);
  const messages = useMessageStore((state) => state.messages);
  const greptileResults = messages.filter((msg) => msg.isGreptileResult);

  const sendToWebSocket = useCallback((type, content) => {
    if (socketRef.current && socketRef.current.readyState === WebSocket.OPEN) {
      socketRef.current.send(JSON.stringify({ type, content }));
    } else {
      console.error("WebSocket is not connected");
    }
  }, []);

  useEffect(() => {
    let term = null;
    let fitAddon = null;

    // Initialize xterm.js
    if (terminalRef.current) {
      term = new Terminal({
        cursorBlink: true,
        fontSize: 14,
        fontFamily: 'Menlo, Monaco, "Courier New", monospace',
        theme: {
          background: "rgb(24 24 27)",
        },
      });

      fitAddon = new FitAddon();
      term.loadAddon(fitAddon);

      term.open(terminalRef.current);
      fitAddon.fit();
      terminalInstanceRef.current = term;

      // Adjust terminal size when window resizes
      const resizeObserver = new ResizeObserver(() => {
        if (fitAddon) {
          fitAddon.fit();
        }
      });
      resizeObserver.observe(terminalRef.current);
    }

    // Create WebSocket connection to Golang server
    const socket = new WebSocket("wss://shell.openagents.com/ws");
    socketRef.current = socket;

    socket.onopen = () => {
      console.log("WebSocket connection established");
      if (term) {
        term.writeln("Connected to EC2 instance");
      }
    };

    socket.onmessage = (event) => {
      const data = JSON.parse(event.data);
      console.log("Received message:", data);

      if (term) {
        switch (data.type) {
          case "connection":
          case "shell_command_result":
            term.writeln(data.content);
            break;
          default:
            console.log("Unknown message type:", data.type);
            term.writeln(`Received: ${data.content}`);
        }
      }
    };

    socket.onerror = (error) => {
      console.error("WebSocket error:", error);
      if (term) {
        term.writeln("Error: Unable to connect to EC2 instance");
      }
    };

    socket.onclose = () => {
      console.log("WebSocket connection closed");
      if (term) {
        term.writeln("Disconnected from EC2 instance");
      }
    };

    // Clean up on unmount
    return () => {
      if (terminalInstanceRef.current) {
        terminalInstanceRef.current.dispose();
      }
      if (socketRef.current) {
        socketRef.current.close();
      }
      if (resizeObserver) {
        resizeObserver.disconnect();
      }
    };
  }, []);

  // React to changes in messages
  useEffect(() => {
    if (messages.length > 0) {
      const lastMessage = messages[messages.length - 1];
      console.log("Last message:", lastMessage);
      if (!lastMessage.isUser && lastMessage.isComplete) {
        sendToWebSocket("anthropic_result", lastMessage.content);
      }
    }
  }, [messages, sendToWebSocket]);

  // React to changes in greptileResults
  useEffect(() => {
    if (greptileResults.length > 0) {
      const latestResult = greptileResults[greptileResults.length - 1];
      sendToWebSocket("greptile_result", latestResult.content);
    }
  }, [greptileResults, sendToWebSocket]);

  return <div ref={terminalRef} className="w-full h-full" />;
}
