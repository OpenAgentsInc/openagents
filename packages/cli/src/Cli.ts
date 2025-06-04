import { Args, Command, Options } from "@effect/cli"
import { TodosClient } from "./TodosClient.js"

const todoArg = Args.text({ name: "todo" }).pipe(
  Args.withDescription("The message associated with a todo")
)

const todoId = Options.integer("id").pipe(
  Options.withDescription("The identifier of the todo")
)

const add = Command.make("add", { todo: todoArg }).pipe(
  Command.withDescription("Add a new todo"),
  Command.withHandler(({ todo }) => TodosClient.create(todo))
)

const done = Command.make("done", { id: todoId }).pipe(
  Command.withDescription("Mark a todo as done"),
  Command.withHandler(({ id }) => TodosClient.complete(id))
)

const list = Command.make("list").pipe(
  Command.withDescription("List all todos"),
  Command.withHandler(() => TodosClient.list)
)

const remove = Command.make("remove", { id: todoId }).pipe(
  Command.withDescription("Remove a todo"),
  Command.withHandler(({ id }) => TodosClient.remove(id))
)

const command = Command.make("todo").pipe(
  Command.withSubcommands([add, done, list, remove])
)

export const cli = Command.run(command, {
  name: "Todo CLI",
  version: "0.0.0"
})
