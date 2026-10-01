// Cart totals for the checkout page.
export function cartTotal(order) {
  const items = order.items ?? [];
  const discount = order.loyaltyAccount.discount;
  return items.reduce((sum, item) => sum + item.price, 0) * (1 - discount);
}
