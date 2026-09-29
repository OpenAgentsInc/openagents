"""The application: routes wired to handlers."""

from api.handlers import orders, users


ROUTES = {
    "GET /users/{id}": users.get_user,
    "POST /users": users.create_user,
    "GET /orders/{id}": orders.get_order,
    "POST /orders": orders.create_order,
}


def handle(method, path, body):
    for pattern, handler in ROUTES.items():
        verb, template = pattern.split(" ", 1)
        if verb != method:
            continue
        if template.split("/")[1] == path.split("/")[1]:
            return handler(path, body)
    return 404, {"error": "no route"}
