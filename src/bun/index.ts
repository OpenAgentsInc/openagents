import { BrowserWindow } from "electrobun/bun";

// Create the main application window
const mainWindow = new BrowserWindow({
  title: "OpenAgents",
  url: "views://mainview/index.html",
  frame: {
    width: 1200,
    height: 800,
    x: 200,
    y: 200,
  },
});

console.log("Hello Electrobun app started!");
