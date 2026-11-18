import { Engine } from "./engine.js";
import type { EngineOptions } from "./types.js";

let enginePromise: Promise<Engine> | undefined;

/**
 * Load the wasm module once and reuse it. Later calls ignore their options
 * and return the same engine.
 */
export function loadEngine(options: EngineOptions = {}): Promise<Engine> {
    if (!enginePromise) {
        enginePromise = Engine.load(options).catch((error: unknown) => {
            enginePromise = undefined;
            throw error;
        });
    }
    return enginePromise;
}
