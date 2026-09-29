import { warm } from "./cache.js";

warm(new Map()).then(() => console.log("ready"));
