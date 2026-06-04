# dw9714 — C → Rust migration of the VCM lens-actuator driver

A self-contained Rust workspace that ports the Intel camera **dw9714**
voice-coil-motor (VCM) lens-actuator driver from C to safe, idiomatic Rust.

It mirrors the logic of:

- `drivers/media/i2c/dw9714.c`
- `include/media/dw9714.h`

**The original C is not modified.** This workspace lives alongside it under
`rust/dw9714/` and is intended to be swapped in incrementally (see the
migration plan below).

## Crates

| Crate          | Purpose |
|----------------|---------|
| `dw9714-sys`   | Raw, **manually written** `unsafe extern "C"` FFI bindings: faithful `#[repr(C)]` mirrors of the C structs, constants, the `VCM_VAL` macro (as a `const fn`), errno values, and the kernel function signatures the driver depends on. Not bindgen-generated. |
| `dw9714`       | The safe, idiomatic wrapper. 100% safe Rust business logic ported from the C driver, with RAII lifecycle handles, `Result<T, DW9714Error>` error handling, and strong newtypes for positions/register words. |

## Safety model

- The safe crate is compiled with `#![forbid(unsafe_code)]` (when the `c-abi`
  feature is off) — the ported logic provably contains **no `unsafe`**.
- All hardware access is funnelled through the `Dw9714Bus` trait. Tests use the
  in-memory `MockI2cBus`; the kernel uses a callback-backed bus.
- With the `c-abi` feature, `unsafe` is `deny`-ed everywhere except the single
  `ffi` module (the FFI boundary shim), where every `unsafe` block carries a
  `SAFETY:` contract.
- `Send`/`Sync` are only ever auto-derived from the bus type — never
  `unsafe impl`-ed — so they are granted strictly when provably correct.

## C → Rust mapping

| C function                | Rust equivalent                              |
|---------------------------|----------------------------------------------|
| `dw9714_i2c_write`        | `Dw9714::write_word` (private, with retry)   |
| `dw9714_t_focus_vcm`      | `Dw9714::set_position`                        |
| `dw9714_set_ctrl`         | `Dw9714::set_ctrl`                            |
| `dw9714_init_controls`    | `Dw9714::init`                               |
| `dw9714_probe` / `remove` | `Dw9714::probe` / `Drop for Dw9714`          |
| `dw9714_open` / `close`   | `Dw9714::open` / `Drop for PowerGuard` (RAII)|
| `dw9714_runtime_suspend`  | `Dw9714::runtime_suspend`                     |
| `dw9714_runtime_resume`   | `Dw9714::runtime_resume`                      |
| `VCM_VAL(data, s)`        | `LensPosition::register_value` / `dw9714_sys::VCM_VAL` |

## Building & testing

```sh
cd rust/dw9714

cargo test                       # safe-crate unit + integration tests
cargo test --features c-abi      # also exercise the C-ABI shim
cargo clippy --all-targets -- -D warnings
cargo clippy --all-targets --features c-abi -- -D warnings
cargo bench                      # Criterion benchmarks (lens-position set cycle)
```

No real hardware or kernel headers are required — the mock I2C backend stands in
for the bus.

> **MSRV note:** built and tested with Rust 1.83. The `Cargo.lock` pins
> `clap` (a transitive Criterion dev-dependency) to a 1.83-compatible version.

## C-ABI usage (incremental swap-in)

With `--features c-abi`, the crate exports `#[no_mangle] extern "C"` symbols.
The C side supplies a callback table (`dw9714_c_ops`) for I2C / GPIO / power /
delay and drives the Rust logic:

```c
struct dw9714_handle *h =
    dw9714_rust_create(addr, gpio_xsd, has_sensor_dev, &ops, user);
dw9714_rust_set_position(h, 512);
dw9714_rust_runtime_resume(h);
dw9714_rust_runtime_suspend(h);
dw9714_rust_destroy(h);
```

## Migration plan (Sprint 1 → Sprint 2)

- **Sprint 1 (this PR):** stand up the parallel Rust workspace, port the logic
  with full test + benchmark coverage, and expose it behind the `c-abi`
  feature. The kernel C driver is untouched and remains the production path.
- **Sprint 2:** build the safe crate as a `staticlib`, link it into the kernel
  module, and route the hot paths (`dw9714_t_focus_vcm`, runtime suspend/resume)
  through the `dw9714_rust_*` entry points behind a Kconfig/feature toggle,
  validating parity on hardware before removing the C implementations.
