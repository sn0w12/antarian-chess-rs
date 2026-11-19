import type { EngineOptions } from "./types.js";

/** Raw exports of `chess_wasm.wasm`. Not part of the public API. */
export interface WasmExports {
    readonly memory: WebAssembly.Memory;
    alloc(length: number): number;
    dealloc(pointer: number, length: number): void;
    result_ptr(): number;
    game_create(): number;
    game_destroy(id: number): void;
    game_reset(id: number): number;
    game_load(id: number, pointer: number, length: number): number;
    game_state(id: number): number;
    game_legal_moves(id: number, from: number): number;
    game_play(
        id: number,
        from: number,
        to: number,
        promotion: number
    ): number;
    game_undo(id: number): number;
    game_best_move(id: number, depth: number): number;
    game_play_best(id: number, depth: number): number;
    bot_move(pointer: number, length: number, depth: number): number;
}

export async function instantiateWasm(
    options: EngineOptions
): Promise<WasmExports> {
    let bytes: BufferSource | undefined = options.wasmBytes;

    if (!bytes) {
        const url = options.wasmUrl
            ? new URL(options.wasmUrl, import.meta.url)
            : new URL("./chess_wasm.wasm", import.meta.url);
        const response = await fetch(url);
        if (!response.ok) {
            throw new Error(
                `failed to load ${url}: ${response.status} ${response.statusText}`
            );
        }
        bytes = await response.arrayBuffer();
    }

    const { instance } = await WebAssembly.instantiate(bytes, {});
    return instance.exports as unknown as WasmExports;
}
