declare module 'react-server-dom-webpack/client.edge' {
  export interface Options {
    serverConsumerManifest: ServerConsumerManifest
    nonce?: string
    encodeFormAction?: EncodeFormActionCallback
    temporaryReferences?: TemporaryReferenceSet
    findSourceMapURL?: FindSourceMapURLCallback
    replayConsoleLogs?: boolean
    environmentName?: string
  }

  export type EncodeFormActionCallback = <A>(
    id: any,
    args: Promise<A>
  ) => ReactCustomFormAction

  export type ReactCustomFormAction = {
    name?: string
    action?: string
    encType?: string
    method?: string
    target?: string
    data?: null | FormData
  }

  export type ImportManifestEntry = {
    id: string | number
    // chunks is a double indexed array of chunkId / chunkFilename pairs
    chunks: ReadonlyArray<string>
    name: string
    async?: boolean
  }

  export type ServerManifest = {
    [id: string]: ImportManifestEntry
  }

  export interface ServerConsumerManifest {
    moduleMap: ServerConsumerModuleMap
    moduleLoading: ModuleLoading | null
    serverModuleMap: null | ServerManifest
  }

  export interface ServerConsumerModuleMap {
    [clientId: string]: {
      [clientExportName: string]: ImportManifestEntry
    }
  }

  export interface ModuleLoading {
    prefix: string
    crossOrigin?: 'use-credentials' | ''
  }

  type TemporaryReferenceSet = Map<string, unknown>

  export type CallServerCallback = (
    id: string,
    args: unknown[]
  ) => Promise<unknown>

  export type FindSourceMapURLCallback = (
    fileName: string,
    environmentName: string
  ) => null | string

  export function createFromFetch<T>(
    promiseForResponse: Promise<Response>,
    options?: Options
  ): Promise<T>

  export function createFromReadableStream<T>(
    stream: ReadableStream,
    options?: Options
  ): Promise<T>

  export function createServerReference(
    id: string,
    callServer: CallServerCallback
  ): (...args: unknown[]) => Promise<unknown>

  export function createTemporaryReferenceSet(
    ...args: unknown[]
  ): TemporaryReferenceSet

  export function encodeReply(
    value: unknown,
    options?: {
      temporaryReferences?: TemporaryReferenceSet
      signal?: AbortSignal
    }
  ): Promise<string | FormData>
}


declare module "react-server-dom-webpack/server.edge" {
  export type ImportManifestEntry = {
    id: string | number;
    // chunks is a double indexed array of chunkId / chunkFilename pairs
    chunks: ReadonlyArray<string>;
    name: string;
    async?: boolean;
  };

  export type ClientManifest = {
    [id: string]: ImportManifestEntry;
  };

  export type ServerManifest = {
    [id: string]: ImportManifestEntry;
  };

  export type TemporaryReferenceSet = WeakMap<any, string>;

  export function renderToReadableStream(
    model: any,
    webpackMap: ClientManifest,
    options?: {
      temporaryReferences?: TemporaryReferenceSet;
      environmentName?: string | (() => string);
      filterStackFrame?: (url: string, functionName: string) => boolean;
      onError?: (error: unknown) => void;
      onPostpone?: (reason: string) => void;
      signal?: AbortSignal;
    }
  ): ReadableStream<Uint8Array>;

  export function createTemporaryReferenceSet(
    ...args: unknown[]
  ): TemporaryReferenceSet;

  export function decodeReply<T>(
    body: string | FormData,
    webpackMap: ServerManifest,
    options?: {
      temporaryReferences?: TemporaryReferenceSet;
    }
  ): Promise<T>;
  export function decodeAction<T>(
    body: FormData,
    serverManifest: ServerManifest
  ): Promise<() => T> | null;
  export function decodeFormState<S>(
    actionResult: S,
    body: FormData,
    serverManifest: ServerManifest
  ): Promise<unknown | null>;

  export function registerServerReference<T>(
    reference: T,
    id: string,
    exportName: string | null
  ): unknown;

  export function createClientModuleProxy(moduleId: string): unknown;
}
