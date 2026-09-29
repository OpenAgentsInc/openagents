"""Order handlers."""

from api.models.order import Order

ORDERS = {}


def get_order(path, _body):
    order_id = path.rsplit("/", 1)[-1]
    order = ORDERS.get(order_id)
    if order is None:
        return 404, {"error": "no such order"}
    return 200, order.to_dict()


def create_order(_path, body):
    order = Order(id=str(len(ORDERS) + 1), user_id=body["user_id"], total_cents=body["total_cents"])
    ORDERS[order.id] = order
    return 201, order.to_dict()
