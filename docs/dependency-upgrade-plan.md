# Dependency upgrade plan

Written 2026-09-04. Goal: bring every dependency in `Cargo.toml` to its latest release.

## Baseline

Checked on Rust 1.95.0 before any changes:

- `cargo check` and `cargo check --no-default-features --features mysql,zephyr` pass.
- `cargo test` passes (2 tests).
- `cargo check --features zip,img_processing` fails. turbojpeg 0.4 was built against an
  older `image` than the 0.24 we resolve, so `compress_image` in
  `src/image_compression.rs` no longer type-checks. This is a pre-existing break.

## Current vs latest

| Crate | Now | Latest | Breaking |
|---|---|---|---|
| axum | 0.6 | 0.8.9 | yes, drives most of the work |
| axum-extra | 0.4 | 0.12.6 | yes, typed-routing path syntax |
| http / hyper | 0.2 / 0.14 | 1.5 / 1.11 | yes, moves with axum |
| tower / tower-http | 0.4 / 0.2 | 0.5 / 0.7 | yes |
| sqlx | 0.7 | 0.9.0 | feature names; needs Rust 1.94 |
| validator | 0.14 | 0.21 | `field_errors()` key type |
| zip | 0.6 | 8.6 | `FileOptions` API, heavy default features |
| image / turbojpeg | 0.24 / 0.4 | 0.25 / 1.5 | `ImageReader` path |
| serial_test | 0.6 | 4.0 | dev-dep only |
| async-session | 3.0 | 3.0 | already latest, unmaintained since 2021 |
| maud fork | 0.23 (rev e39cef7) | same | no newer commit on the fork |

Everything else (tokio, serde, serde_json, chrono, uuid, tracing, tracing-appender,
tracing-subscriber, futures, bytes, backtrace, paste, const-random, tokio-util) is
caret-pinned and already resolves to latest. Raising the minimums is cosmetic.

## Phases

Each phase is one commit. Each must build and pass `cargo test` with both the default
(`sqlite`) features and `--no-default-features --features mysql,zephyr`.

### 1. Housekeeping

- Bump the non-breaking minimums listed above.
- Drop `hyper` as a direct dependency. Nothing in `src/` uses it.
- Bump `serial_test` to 4. It is only exported for downstream tests.

### 2. axum 0.8 ecosystem

Upgrade together, since they are coupled: axum 0.8, axum-extra 0.12, http 1, tower 0.5,
tower-http, plus a new direct dependency `headers = "0.4"` for the typed `Cookie` header
(axum dropped its `headers` feature in 0.7).

Pin tower-http to 0.6, not 0.7. axum-extra 0.12 depends on tower-http 0.6, so 0.7 would
compile two copies.

Code changes:

- `src/sessions.rs`, `src/theme.rs`: `axum::headers::*` becomes `headers::*`.
  `session_middleware` loses its `B` generic; `Next` and `Request` are concrete in 0.7+.
- `src/html/mod.rs`: `HtmlMiddleware<B, T, R>` and `AssociatedMiddleware<B>` lose the `B`
  parameter. Adjust `default_layers!` in `src/router.rs` to match; downstream calls are
  unaffected because the macro lives here. Remove `#[axum::async_trait]` from `NoRoute`;
  axum 0.8 uses native async fn in traits.
- `src/html/mod.rs`: `http 1`'s `Extensions::insert` requires `Clone`, so
  `html_context_middleware` now bounds `T: Clone` and `R: Clone`. Extracting
  `Extension<HtmlContextBuilder<T, R>>` in a handler already required this, so it is
  not a new constraint in practice. `NoRoute` derives `Clone`.
- `src/macro_helpers.rs`: remove `#[axum::async_trait]` from the generated
  `FromRequestParts` impl.
- `src/extractors/multipart.rs`: `FromRequest<S, B>` becomes `FromRequest<S>` taking
  `axum::extract::Request`. `Field` still implements `Stream<Item = Result<Bytes, _>>`,
  so `stream_to_file` stays as is.
- `src/router.rs`: `ServeDir` is infallible, so `get_service(...).handle_error(...)`
  becomes `nest_service(path, ServeDir::new(path))`.
- `src/tests/helpers.rs`: `HttpBody::data()` is gone; use `axum::body::to_bytes`.
  Replace `async_session::async_trait` with a native `async fn` in `RouterExt`.

### 3. sqlx 0.9

- `runtime-tokio-rustls` is split into `runtime-tokio` and `tls-rustls`.
- Requires Rust 1.94.
- Confirm that binding `Utc::now()` and reading `NaiveDateTime` in `src/sessions.rs`
  still type-check on both backends.

### 4. validator 0.21

- `ValidationErrors::field_errors()` now keys by `Cow<'static, str>`. Add `.to_string()`
  on the key in `UserSession::validation_errors`.
- `src/validation.rs` is unaffected.

### 5. Optional features

zip 8:

- `Default::default()` for file options no longer infers. Use `SimpleFileOptions::default()`
  in `start_file` and `add_directory`.
- Set `default-features = false, features = ["deflate"]`. The defaults pull in aes, bzip2,
  lzma, zstd, and xz.

image 0.25 and turbojpeg 1.5:

- `image::io::Reader` becomes `image::ImageReader`.
- turbojpeg 1.5's `image` feature accepts 0.24 to 0.25, so the two line up. This also
  fixes the pre-existing break noted in the baseline.
- Verify with `cargo check --features zip,img_processing`. Requires the system
  `libturbojpeg` (installed here via Homebrew `jpeg-turbo`).

### 6. Downstream changes for muxa-template

Apps built on muxa will need these edits after picking up the new version:

- Route strings in `routes!` change from `/users/:id` to `/users/{id}`.
- `Template` structs that implement `FromRequestParts` must drop `#[async_trait]`.
- `Multipart<F>` and `session_middleware` no longer take body generics.
- Any direct use of `axum::headers` moves to the `headers` crate.
- `Template` structs must derive `Clone`, as must the `NamedRoute` type (`routes!`
  already derives it).

## Verification matrix

```sh
cargo test
cargo test --no-default-features --features mysql,zephyr
cargo check --features zip,img_processing
cargo clippy --all-targets
```

## Deliberately out of scope

- **maud.** The fork's only branch is the pinned rev from July 2022, based on maud 0.23.
  muxa does not use maud's axum integration, so the fork does not block anything above.
  Rebasing zephyr support onto maud 0.27 is a separate project in that repo.
- **async-session.** Already at its latest release, but unmaintained and it drags in
  async-std and 2021-era crypto crates. muxa uses it as a serializable map plus cookie-id
  hashing, a small surface that could be replaced with a local type. Separate decision.
