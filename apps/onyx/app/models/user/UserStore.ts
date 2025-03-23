import { Instance, SnapshotIn, SnapshotOut, types } from "mobx-state-tree"
import { withSetPropAction } from "../_helpers/withSetPropAction"

export const UserStoreModel = types
  .model("UserStore")
  .props({
    pushToken: types.optional(types.string, ""),
  })
  .actions(withSetPropAction)
  .actions((self) => ({
    setPushToken(token: string) {
      self.pushToken = token
    },

    clearPushToken() {
      self.pushToken = ""
    }
  }))
  .views((self) => ({
    get hasPushToken() {
      return !!self.pushToken
    }
  }))

export interface UserStore extends Instance<typeof UserStoreModel> { }
export interface UserStoreSnapshotOut extends SnapshotOut<typeof UserStoreModel> { }
export interface UserStoreSnapshotIn extends SnapshotIn<typeof UserStoreModel> { }

export const createUserStoreDefaultModel = () =>
  UserStoreModel.create({
    pushToken: "",
  })
