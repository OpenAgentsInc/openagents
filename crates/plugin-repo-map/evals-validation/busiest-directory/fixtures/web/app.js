document.querySelector("h1").addEventListener("click", () => {
  fetch("/api/health").then((r) => r.json()).then(console.log);
});
