"""User handlers."""

from api.models.user import User

USERS = {}


def get_user(path, _body):
    user_id = path.rsplit("/", 1)[-1]
    user = USERS.get(user_id)
    if user is None:
        return 404, {"error": "no such user"}
    return 200, user.to_dict()


def create_user(_path, body):
    user = User(id=str(len(USERS) + 1), name=body["name"])
    USERS[user.id] = user
    return 201, user.to_dict()
