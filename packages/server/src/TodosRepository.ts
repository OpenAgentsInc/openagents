import { Todo, TodoId, TodoNotFound } from "@template/domain/TodosApi"
import { Effect, HashMap, Ref } from "effect"

export class TodosRepository extends Effect.Service<TodosRepository>()("api/TodosRepository", {
  effect: Effect.gen(function*() {
    const todos = yield* Ref.make(HashMap.empty<TodoId, Todo>())

    const getAll = Ref.get(todos).pipe(
      Effect.map((todos) => Array.from(HashMap.values(todos)))
    )

    function getById(id: number): Effect.Effect<Todo, TodoNotFound> {
      return Ref.get(todos).pipe(
        Effect.flatMap(HashMap.get(id)),
        Effect.catchTag("NoSuchElementException", () => new TodoNotFound({ id }))
      )
    }

    function create(text: string): Effect.Effect<Todo> {
      return Ref.modify(todos, (map) => {
        const id = TodoId.make(HashMap.reduce(map, 0, (max, todo) => todo.id > max ? todo.id : max))
        const todo = new Todo({ id, text, done: false })
        return [todo, HashMap.set(map, id, todo)]
      })
    }

    function complete(id: number): Effect.Effect<Todo, TodoNotFound> {
      return getById(id).pipe(
        Effect.map((todo) => new Todo({ ...todo, done: true })),
        Effect.tap((todo) => Ref.update(todos, HashMap.set(todo.id, todo)))
      )
    }

    function remove(id: number): Effect.Effect<void, TodoNotFound> {
      return getById(id).pipe(
        Effect.flatMap((todo) => Ref.update(todos, HashMap.remove(todo.id)))
      )
    }

    return {
      getAll,
      getById,
      create,
      complete,
      remove
    } as const
  })
}) {}
