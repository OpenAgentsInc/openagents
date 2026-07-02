import { Schema as S } from "effect"

export const KhalaCodeFoldkitModel = S.Struct({
  label: S.String,
  mountId: S.String,
  pingCount: S.Number,
})
export type KhalaCodeFoldkitModel = typeof KhalaCodeFoldkitModel.Type

export const initialKhalaCodeFoldkitModel = (
  mountId: string,
): KhalaCodeFoldkitModel => ({
  label: "Foldkit skeleton",
  mountId,
  pingCount: 0,
})

