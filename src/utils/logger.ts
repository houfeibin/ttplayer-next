/**
 * Centralized error logging for IPC calls and async operations.
 * Replaces silent `catch {}` blocks with structured logging.
 */

const TAG = '[TTPlayer]';

/** Log IPC/async errors with context. Use in .catch() chains. */
export function logError(context: string, err: unknown): void {
  const msg = err instanceof Error ? err.message : String(err);
  console.error(`${TAG} ${context}: ${msg}`);
}

/** Log + swallow — for non-critical operations where we want to continue. */
export function logWarn(context: string, err: unknown): void {
  const msg = err instanceof Error ? err.message : String(err);
  console.warn(`${TAG} ${context}: ${msg}`);
}

/** Log + rethrow — for critical operations that should propagate. */
export function logAndThrow(context: string, err: unknown): never {
  logError(context, err);
  throw err;
}
