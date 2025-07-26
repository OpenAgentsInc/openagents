import { z } from "zod";

// Define the structure of user data we'll store
export const UserSchema = z.object({
  id: z.string(),
  email: z.string().email(),
  name: z.string().optional(),
  avatar: z.string().url().optional(),
  githubId: z.string(),
  githubUsername: z.string(),
});

export type User = z.infer<typeof UserSchema>;

// Subject configuration for OpenAuth
// This defines what claims will be included in the JWT
export const subjects = {
  user: UserSchema,
};

export type Subjects = typeof subjects;