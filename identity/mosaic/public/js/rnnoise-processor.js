/**
 * RNNoise AudioWorklet Processor
 * Processes mic audio through RNNoise WASM for real-time noise suppression.
 * 
 * RNNoise operates on 480-sample frames (10ms at 48kHz).
 * AudioWorklet's process() delivers 128-sample blocks.
 * We buffer input samples and process whenever we accumulate a full frame.
 */
class RNNoiseProcessor extends AudioWorkletProcessor {
  constructor() {
    super();
    this._ready = false;
    this._destroyed = false;

    // Ring buffers for bridging 128-sample blocks → 480-sample RNNoise frames
    this._inputBuf = new Float32Array(480);
    this._outputBuf = new Float32Array(480);
    this._inputPos = 0;   // write cursor into _inputBuf
    this._outputPos = 0;  // read cursor from _outputBuf
    this._outputReady = 0; // samples available in _outputBuf

    this.port.onmessage = (e) => {
      if (e.data.type === 'wasm-module') {
        this._initWasm(e.data.module);
      } else if (e.data.type === 'destroy') {
        this._cleanup();
      }
    };
  }

  async _initWasm(wasmModule) {
    try {
      // The Jitsi RNNoise WASM exports its own memory ('c') and needs two imports:
      //   a.a = _emscripten_resize_heap (grow memory)
      //   a.b = _emscripten_memcpy_big (fast memcpy via TypedArray.copyWithin)
      let wasmMemory = null;
      let HEAPU8 = null;

      const updateViews = () => {
        HEAPU8 = new Uint8Array(wasmMemory.buffer);
        this._HEAPF32 = new Float32Array(wasmMemory.buffer);
      };

      const instance = await WebAssembly.instantiate(wasmModule, {
        a: {
          a: (requestedSize) => {
            // _emscripten_resize_heap — grow memory
            const oldSize = HEAPU8.length;
            const maxHeapSize = 2147483648;
            requestedSize = requestedSize >>> 0;
            if (requestedSize > maxHeapSize) return false;
            for (let cutDown = 1; cutDown <= 4; cutDown *= 2) {
              let overGrown = oldSize * (1 + 0.2 / cutDown);
              overGrown = Math.min(overGrown, requestedSize + 100663296);
              const newSize = Math.min(maxHeapSize,
                (Math.max(requestedSize, overGrown) + 65535) & ~65535);
              try {
                wasmMemory.grow((newSize - wasmMemory.buffer.byteLength + 65535) >>> 16);
                updateViews();
                return true;
              } catch (e) { /* try next */ }
            }
            return false;
          },
          b: (dest, src, num) => {
            // _emscripten_memcpy_big — fast memcpy
            HEAPU8.copyWithin(dest, src, src + num);
          }
        }
      });

      const exports = instance.exports;
      wasmMemory = exports.c; // exported Memory
      updateViews();

      // Call __wasm_call_ctors to initialize (export 'd')
      if (exports.d) exports.d();

      this._malloc = exports.g;
      this._free = exports.i;
      this._rnnoise_create = exports.f;
      this._rnnoise_destroy = exports.h;
      this._rnnoise_process_frame = exports.j;
      this._wasmMemory = wasmMemory;

      // Create denoiser state
      this._state = this._rnnoise_create();

      // Allocate input/output buffers in WASM heap (480 floats = 1920 bytes each)
      this._wasmInputPtr = this._malloc(480 * 4);
      this._wasmOutputPtr = this._malloc(480 * 4);

      this._ready = true;
      this.port.postMessage({ type: 'ready' });
    } catch (err) {
      this.port.postMessage({ type: 'error', message: err.message });
    }
  }

  _cleanup() {
    this._destroyed = true;
    if (this._state) {
      this._rnnoise_destroy(this._state);
      this._state = null;
    }
    if (this._wasmInputPtr) {
      this._free(this._wasmInputPtr);
      this._free(this._wasmOutputPtr);
      this._wasmInputPtr = null;
      this._wasmOutputPtr = null;
    }
    this._ready = false;
  }

  process(inputs, outputs) {
    if (this._destroyed) return false;

    const input = inputs[0]?.[0];
    const output = outputs[0]?.[0];
    if (!input || !output) return true;

    // If WASM not ready yet, pass through
    if (!this._ready) {
      output.set(input);
      return true;
    }

    // Feed input samples into the ring buffer, process when we have 480
    for (let i = 0; i < input.length; i++) {
      this._inputBuf[this._inputPos++] = input[i];

      if (this._inputPos === 480) {
        this._processFrame();
        this._inputPos = 0;
      }
    }

    // Read processed samples from output buffer
    for (let i = 0; i < output.length; i++) {
      if (this._outputReady > 0) {
        output[i] = this._outputBuf[this._outputPos++];
        this._outputReady--;
        if (this._outputPos >= 480) this._outputPos = 0;
      } else {
        output[i] = 0; // underrun — silence
      }
    }

    return true;
  }

  _processFrame() {
    // Refresh heap view in case memory grew
    if (this._HEAPF32.buffer !== this._wasmMemory.buffer) {
      this._HEAPF32 = new Float32Array(this._wasmMemory.buffer);
    }

    // RNNoise expects float32 samples scaled to roughly [-32768, 32767]
    const inIdx = this._wasmInputPtr >> 2;
    for (let i = 0; i < 480; i++) {
      this._HEAPF32[inIdx + i] = this._inputBuf[i] * 32768;
    }

    // Process — returns VAD probability (0..1), output written to wasmOutputPtr
    this._rnnoise_process_frame(this._state, this._wasmOutputPtr, this._wasmInputPtr);

    // Read output, scale back to [-1, 1]
    const outIdx = this._wasmOutputPtr >> 2;
    this._outputPos = 0;
    for (let i = 0; i < 480; i++) {
      this._outputBuf[i] = this._HEAPF32[outIdx + i] / 32768;
    }
    this._outputReady = 480;
  }
}

registerProcessor('rnnoise-processor', RNNoiseProcessor);
