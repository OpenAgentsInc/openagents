/**
 * Type to represent a deep readonly object (RxDB returns readonly objects)
 */
export type DeepReadonlyObject<T> = {
  readonly [K in keyof T]: T[K] extends Array<infer U>
  ? ReadonlyArray<DeepReadonlyObject<U>>
  : T[K] extends object
  ? DeepReadonlyObject<T[K]>
  : T[K];
};
