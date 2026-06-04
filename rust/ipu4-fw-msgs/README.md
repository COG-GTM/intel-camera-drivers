# ipu4-fw-msgs — safe Rust port of the IPU4 ISYS firmware message marshaling

A self-contained Rust workspace porting the IPU4 (Intel Image Processing Unit 4)
ISYS firmware message marshaling / unmarshaling module from C to safe, idiomatic
Rust.

It mirrors `drivers/media/pci/intel-ipu4/intel-ipu4-isys-fw-msgs.{c,h}`. The C
code is **not** modified — this is an additive, parallel implementation that is
byte-for-byte ABI-compatible with the firmware.

## Crates

| Crate | Description |
|-------|-------------|
| [`ipu4-fw-msgs-sys`](ipu4-fw-msgs-sys) | Raw `#[repr(C)]` structs, enums and `extern "C"` function signatures faithfully mirroring the firmware ABI. |
| [`ipu4-fw-msgs`](ipu4-fw-msgs) | Idiomatic, memory-safe message construction, serialization and deserialization: builder-pattern construction, `Result`-based validation, `&[u8]`-based (de)serialization with **zero** raw pointer casts, and strong newtypes for pin / stream / buffer indices. |

## What was ported

* Stream-open payload (`StreamCfgData`) incl. the `isa_cfg` bitfield, cropping,
  and per-pin descriptors, plus `intel_ipu4_isys_set_fw_params` (the
  data-type → bits-per-pixel table fill).
* Capture payload (`FrameBuffSet`).
* Command tokens for stream open/start/capture/stop/flush/close
  (`SendQueueToken`, both "complex" and "simple" forms) and proxy writes
  (`ProxySendQueueToken`).
* Response parsing (`RespInfo`, `ProxyRespInfo`, `ErrorInfo`) and the queue
  routing arithmetic.

## Memory-safety improvements over the C code

The C driver marshals by casting between struct pointers and byte buffers
(`(struct foo *)buf`). This port eliminates the three classic hazards of that
pattern:

* **Misaligned access** — every field is read/written through bounds-checked,
  endianness-explicit cursors, so buffer alignment is irrelevant.
* **Buffer overruns** — every access is length-checked; a short buffer returns
  `Error::BufferTooSmall` instead of reading/writing out of bounds.
* **Use-after-free / type confusion** — ownership is expressed with owned values
  and borrowed slices, and strong newtypes (`InputPinId`, `OutputPinId`,
  `StreamHandle`, `BufferId`, `CssVirtualAddress`) make it a compile error to mix
  a pin index with a stream handle or a buffer id with an address.

All marshaling logic is 100% safe Rust; `unsafe` appears only in the optional
C ABI shim.

## C ABI exports

Build with `--features c-abi` to expose the safe implementation through the
original C ABI via `#[no_mangle] extern "C"` functions (see
[`src/c_abi.rs`](ipu4-fw-msgs/src/c_abi.rs)).

## Testing

```sh
cargo test          # round-trip, boundary, malformed-input, golden-file parity
cargo clippy --all-targets --all-features -- -D warnings
cargo bench         # Criterion construction + (de)serialization throughput
```

Golden-file parity tests assert byte-exact equivalence against reference
messages emitted by a small C program that `#include`s the unmodified kernel
headers (see [`tests/golden/gen`](ipu4-fw-msgs/tests/golden/gen); run `make`
there to regenerate the `*.bin` fixtures).

> The dev/bench dependency tree is pinned (see `Cargo.lock`) to versions that
> build on the toolchain in this repo (rustc 1.83).
