import 'dotenv/config'
import type { Config } from 'drizzle-kit'

export default {
  schema: './src/schema.ts',
  out: './drizzle',
  dialect: 'mysql',
  dbCredentials: {
    host: process.env.DATABASE_HOST!,
    user: process.env.DATABASE_USERNAME!,
    password: process.env.DATABASE_PASSWORD!,
    database: process.env.DATABASE_NAME || 'openagents_relay',
    ssl: {
      rejectUnauthorized: false
    }
  },
  verbose: true,
  strict: true,
} satisfies Config