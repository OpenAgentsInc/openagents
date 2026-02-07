import { Effect, Option } from 'effect';
import { tryPromise } from './tryPromise';

import type { StorageReader, StorageWriter } from 'convex/server';
import type { Id } from '../_generated/dataModel';

export interface EffectStorageReader {
  readonly getUrl: (storageId: Id<'_storage'>) => Effect.Effect<Option.Option<string>, Error>;
}

export class EffectStorageReaderImpl implements EffectStorageReader {
  constructor(private storage: StorageReader) {}

  getUrl(storageId: Id<'_storage'>): Effect.Effect<Option.Option<string>, Error> {
    return tryPromise(() => this.storage.getUrl(storageId)).pipe(Effect.map(Option.fromNullable));
  }
}

export interface EffectStorageWriter extends EffectStorageReader {
  readonly generateUploadUrl: () => Effect.Effect<string, Error>;
  readonly delete: (storageId: Id<'_storage'>) => Effect.Effect<void, Error>;
}

export class EffectStorageWriterImpl implements EffectStorageWriter {
  private reader: EffectStorageReader;

  constructor(private storage: StorageWriter) {
    this.reader = new EffectStorageReaderImpl(storage);
  }

  getUrl(storageId: Id<'_storage'>): Effect.Effect<Option.Option<string>, Error> {
    return this.reader.getUrl(storageId);
  }

  generateUploadUrl(): Effect.Effect<string, Error> {
    return tryPromise(() => this.storage.generateUploadUrl());
  }

  delete(storageId: Id<'_storage'>): Effect.Effect<void, Error> {
    return tryPromise(() => this.storage.delete(storageId)).pipe(Effect.asVoid);
  }
}
