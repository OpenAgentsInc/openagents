import { v } from 'convex/values';

import type { PropertyValidators, Validator } from 'convex/values';

type TableWithFields<TFields extends PropertyValidators> = {
  readonly validator: {
    readonly fields: TFields;
  };
};

/** Build a document validator that includes Convex system fields for a table. */
export const doc = <TTableName extends string, TFields extends PropertyValidators>(
  tableName: TTableName,
  table: TableWithFields<TFields>,
) =>
  v.object({
    _id: v.id(tableName),
    _creationTime: v.number(),
    ...table.validator.fields,
  });

export const nullable = <TValidator extends Validator<any, 'required', any>>(validator: TValidator) =>
  v.union(validator, v.null());
