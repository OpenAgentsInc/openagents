import { betterAuth } from "better-auth";
import Database from "better-sqlite3";

export const auth: ReturnType<typeof betterAuth> = betterAuth({
  database: new Database("./sqlite.db"),
  emailAndPassword: {
    enabled: true
  },
})
