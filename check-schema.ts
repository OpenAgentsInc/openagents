// Check current database schema
import { Client } from "@planetscale/database"
import 'dotenv/config'

const client = new Client({
  host: process.env.DATABASE_HOST!,
  username: process.env.DATABASE_USERNAME!,
  password: process.env.DATABASE_PASSWORD!,
  database: process.env.DATABASE_NAME!,
})

async function checkSchema() {
  console.log('🔍 Checking current database schema...\n')
  
  try {
    // Check channels table structure
    const columnsResult = await client.execute('DESCRIBE channels')
    
    console.log('Current channels table columns:')
    columnsResult.rows.forEach((row: any) => {
      console.log(`  ${row.Field} (${row.Type}) ${row.Null === 'NO' ? 'NOT NULL' : 'NULL'} ${row.Default ? `DEFAULT ${row.Default}` : ''}`)
    })
    
    console.log('\nRequired columns by schema:')
    const required = [
      'id', 'name', 'about', 'picture', 'creator_pubkey', 
      'message_count', 'last_message_at', 'created_at', 'updated_at'
    ]
    
    const existing = columnsResult.rows.map((row: any) => row.Field)
    
    required.forEach(col => {
      const exists = existing.includes(col)
      console.log(`  ${exists ? '✅' : '❌'} ${col}`)
    })
    
  } catch (error) {
    console.error('❌ Error:', error)
  }
}

checkSchema()