import { cast, Instance, SnapshotIn, SnapshotOut, types } from "mobx-state-tree"
import { withSetPropAction } from "../_helpers/withSetPropAction"
import { Repo } from "../types/repo"

// Repo Model
export const RepoModel = types.model("Repo", {
  owner: types.string,
  name: types.string,
  branch: types.string,
})

// GitHub Token Model
export const GithubTokenModel = types.model("GithubToken", {
  id: types.identifier,
  name: types.string,
  token: types.string,
})

export const CoderStoreModel = types
  .model("CoderStore")
  .props({
    error: types.maybeNull(types.string),
    // Keep the old token field for backward compatibility
    githubToken: types.optional(types.string, ""),
    githubTokens: types.array(GithubTokenModel),
    activeTokenId: types.maybeNull(types.string),
    repos: types.array(RepoModel),
    activeRepoIndex: types.maybeNull(types.number),
  })
  .actions(withSetPropAction)
  .actions((self) => ({
    afterCreate() {
      // CRITICAL: If there's an old token, ALWAYS ensure it exists in the new system
      if (self.githubToken) {
        const legacyTokenExists = self.githubTokens.some(t =>
          t.name === "Legacy Token" && t.token === self.githubToken
        )

        if (!legacyTokenExists) {
          const id = `token_${Date.now()}`
          const newToken = GithubTokenModel.create({
            id,
            name: "Legacy Token",
            token: self.githubToken,
          })
          self.githubTokens.push(newToken)
          self.activeTokenId = id
        }
      }
    },

    setError(error: string | null) {
      self.error = error
    },

    // Keep the old method for backward compatibility
    setGithubToken(token: string) {
      self.githubToken = token
      // Also add/update in new system
      if (token) {
        const existingToken = self.githubTokens.find(t => t.name === "Legacy Token")
        if (existingToken) {
          this.updateGithubToken(existingToken.id, "Legacy Token", token)
        } else {
          this.addGithubToken("Legacy Token", token)
        }
      }
    },

    // Token actions
    addGithubToken(name: string, token: string) {
      const id = `token_${Date.now()}`
      const newToken = GithubTokenModel.create({ id, name, token })
      self.githubTokens.push(newToken)
      if (!self.activeTokenId) {
        self.activeTokenId = id
        self.githubToken = token // Keep legacy token in sync
      }
    },

    removeGithubToken(id: string) {
      const index = self.githubTokens.findIndex(t => t.id === id)
      if (index !== -1) {
        const token = self.githubTokens[index]
        
        // If removing active token, update legacy token
        if (self.activeTokenId === id) {
          // Find another token to make active
          const remainingTokens = self.githubTokens.filter((_, i) => i !== index)
          if (remainingTokens.length > 0) {
            self.activeTokenId = remainingTokens[0].id
            self.githubToken = remainingTokens[0].token
          } else {
            self.activeTokenId = null
            // Clear legacy token if we're removing it
            if (token.name === "Legacy Token") {
              self.githubToken = ""
            }
          }
        }

        self.githubTokens = cast(self.githubTokens.filter((_, i) => i !== index))
      }
    },

    updateGithubToken(id: string, name: string, token: string) {
      const index = self.githubTokens.findIndex(t => t.id === id)
      if (index !== -1) {
        self.githubTokens = cast(self.githubTokens.map((t, i) =>
          i === index ? GithubTokenModel.create({ id, name, token }) : t
        ))
        // If updating active token, update legacy token
        if (self.activeTokenId === id) {
          self.githubToken = token
        }
      }
    },

    setActiveTokenId(id: string | null) {
      if (id === null || self.githubTokens.some(t => t.id === id)) {
        self.activeTokenId = id
        // Update legacy token
        if (id) {
          const token = self.githubTokens.find(t => t.id === id)
          if (token) {
            self.githubToken = token.token
          }
        }
      }
    },

    // Repo actions
    addRepo(repo: Repo) {
      const newRepo = RepoModel.create(repo)
      self.repos.push(newRepo)
      if (self.activeRepoIndex === null) {
        self.activeRepoIndex = self.repos.length - 1
      }
    },

    removeRepo(repoToRemove: Repo) {
      const index = self.repos.findIndex(repo =>
        repo.owner === repoToRemove.owner &&
        repo.name === repoToRemove.name &&
        repo.branch === repoToRemove.branch
      )

      if (index !== -1) {
        self.repos = cast(self.repos.filter((_, i) => i !== index))
        if (self.activeRepoIndex === index) {
          self.activeRepoIndex = self.repos.length > 0 ? 0 : null
        } else if (self.activeRepoIndex !== null && self.activeRepoIndex > index) {
          self.activeRepoIndex--
        }
      }
    },

    updateRepo(oldRepo: Repo, newRepo: Repo) {
      const index = self.repos.findIndex(repo =>
        repo.owner === oldRepo.owner &&
        repo.name === oldRepo.name &&
        repo.branch === oldRepo.branch
      )

      if (index !== -1) {
        self.repos = cast(self.repos.map((repo, i) =>
          i === index ? RepoModel.create(newRepo) : repo
        ))
      }
    },

    setActiveRepoByIndex(index: number | null) {
      if (index !== null && (index < 0 || index >= self.repos.length)) {
        return
      }
      self.activeRepoIndex = index
    },

    setActiveRepo(repo: Repo | null) {
      if (!repo) {
        self.activeRepoIndex = null
        return
      }

      const index = self.repos.findIndex(r =>
        r.owner === repo.owner &&
        r.name === repo.name &&
        r.branch === repo.branch
      )

      if (index !== -1) {
        self.activeRepoIndex = index
      }
    }
  }))
  .views((self) => {
    const views = {
      get activeToken() {
        return self.activeTokenId ? self.githubTokens.find(t => t.id === self.activeTokenId) : null
      },

      get githubTokenValue() {
        // CRITICAL: Always return the legacy token if it exists
        return self.githubToken || (views.activeToken ? views.activeToken.token : "") || ""
      },

      get hasGithubToken() {
        return !!(self.githubToken || self.githubTokens.length > 0)
      },

      get activeRepo() {
        return self.activeRepoIndex !== null ? self.repos[self.activeRepoIndex] : null
      }
    }
    return views
  })

export interface CoderStore extends Instance<typeof CoderStoreModel> { }
export interface CoderStoreSnapshotOut extends SnapshotOut<typeof CoderStoreModel> { }
export interface CoderStoreSnapshotIn extends SnapshotIn<typeof CoderStoreModel> { }

export const createCoderStoreDefaultModel = () =>
  CoderStoreModel.create({
    error: null,
    githubToken: "",
    githubTokens: [],
    activeTokenId: null,
    repos: [],
    activeRepoIndex: null,
  })