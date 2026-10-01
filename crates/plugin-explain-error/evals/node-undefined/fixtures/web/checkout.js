import { cartTotal } from "./cart.js";

// Draws the checkout summary.
export function renderCheckout(order, root) {
  const lines = order.items.map((item) => `${item.name}: ${item.price}`);
  root.innerHTML = lines.join("<br>");
  const footer = document.createElement("p");
  footer.className = "total";
  root.appendChild(footer);
  // The total, after any loyalty discount.
  const total =
    cartTotal(order);
  footer.textContent = `Total: ${total.toFixed(2)}`;
}
