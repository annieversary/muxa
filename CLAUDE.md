# CLAUDE.md

This file provides guidance to Claude Code (claude.ai/code) when working with code in this repository.

## What this is

`muxa` is a small, opinionated Rust library layered on top of axum 0.8 and a fork of maud (`annieversary/maud`, pinned by git rev in `Cargo.toml`). It is a library crate: there is no binary and no `main.rs`. `examples/app.rs` is a small app wired up the way muxa expects, kept mainly so the build compiles the `routes!` and `default_layers!` macros, which expand to `muxa::` paths and so cannot be exercised from inside the library. Downstream apps are scaffolded from [muxa-template](https://github.com/annieversary/muxa-template). The readme is candid that this is early-stage and shaped around the author's own workflow.

## Commands

```sh
cargo build
cargo test                       # unit tests live inline in src/ (e.g. src/helpers.rs)
cargo test test_copy_extension   # run a single test by name
cargo run --example app          # the example app, on :3000 (sqlite only)
cargo test --features mysql --no-default-features --features zephyr   # swap sqlite for mysql
cargo test --all-features        # NOTE: enabling both sqlite and mysql will not compile (see below)
cargo clippy
```

`Cargo.lock` is gitignored, so dependency resolution happens fresh on each machine. The first build needs network access to fetch the maud git fork.

## Feature flags

Defined in `Cargo.toml`; defaults are `sqlite` + `zephyr`.

- `sqlite` / `mysql`: mutually exclusive. Both define `type DbPool` in `src/sessions.rs` and select the upsert SQL dialect, so enabling both is a compile error. Pick exactly one.
- `zephyr`: enables `src/css.rs`, which dumps the maud fork's zephyr utility-class inventory to a CSS file.
- `img_processing`: enables `src/image_compression.rs` (turbojpeg + image).
- `zip`: enables `src/zip.rs`.

## Architecture

The library assumes a specific request pipeline. Everything is wired together by the `default_layers!` macro in `src/router.rs`, and most extractors and middleware `expect()` that the earlier layers ran. The order matters:

1. `TraceLayer`, then `Extension<DbPool>` and `Extension<Config>`, plus any app-supplied extensions.
2. `Extension<DbSessionStore>` followed by `sessions::session_middleware`. This loads the session from the `muxa_session` cookie, inserts a `UserSession` into request extensions, and after the handler runs it prunes one-shot data and writes the `Set-Cookie` header.
3. `HtmlMiddleware::html_context_middleware`, which builds an `HtmlContextBuilder<T, R>` and inserts it as an extension. `T` is the app's template type and `R` is its route type; both are pulled out of the request via `FromRequestParts`, so they must be extractable at this point.

### Sessions (`src/sessions.rs`)

Sessions are stored in a `sessions` table (schema in `migrations/sessions.sql`, MySQL syntax; adapt for sqlite). `UserSession` exposes Laravel-style one-request-lifetime helpers: `flash`, `errors` / `validation_errors`, and `old` (previous form input). Each sets a companion "tracker" key. `session_middleware` removes any of these keys whose tracker was not set during the current request, which is how they survive exactly one redirect. Every mutating method on `UserSession` saves to the DB immediately.

A row is written for every request, including ones from visitors that never come back, and nothing removes them on its own. `DbSessionStore::prune_expired` deletes the rows whose `expires` has passed, and `spawn_pruner` runs it on a timer; `default_layers!` builds its own store, so an app has to call `spawn_pruner` itself (see `examples/app.rs`).

### HTML rendering (`src/html/`)

Handlers take `Extension<HtmlContextBuilder<T, R>>`, call `.build(markup)` to get an `HtmlContext`, then chain `with_title`, `with_description`, `with_image`, `section_append` (like Blade's `@push`). `HtmlContext` implements `IntoResponse` when `T: Template<R>`. The app implements `Template` (`head` and `body`, optionally overriding `base`) on its own struct; that struct is the `T` and also must implement `FromRequestParts<()>` and `Clone` so the middleware can construct it per request and store it in the request extensions. `NoRoute` is the placeholder `R` when named routes are not used.

The middleware only inserts the builder when `R` extracts. A request that matched no route, matched a service that is not a named route (a `ServeDir` mount, say), or carries params that fail to parse passes through without one, and the handler's own rejection or the router fallback answers it. Handlers that take `Extension<HtmlContextBuilder<T, R>>` are therefore only reachable on named routes.

### Named routes (`routes!` macro in `src/macro_helpers.rs`)

`routes! { "/users/{id}" => User { id: i64 } ... }` generates, per entry, a `UserPath` struct deriving axum-extra's `TypedPath`, a `route_user(id)` constructor, and one `NamedRoute` enum with variants for every route. `NamedRoute` implements `FromRequestParts` by matching axum's `MatchedPath`, so it doubles as the `R` in `HtmlContext` for "which page am I on" checks (`matches` ignores params, `PartialEq` includes them). It also implements `maud::Render`, so it can be used directly as an `href` in templates. The macro refers to `muxa::` paths, so it only works from a downstream crate, or from `examples/app.rs`.

### Errors (`src/errors.rs`)

`ErrResponse` is the single error type handlers return. It captures `Location::caller()` and a backtrace via `#[track_caller]`, and there is a blanket `From<E: std::error::Error>`, so `?` works on almost anything. Debug builds render the location and backtrace in the HTTP body; release builds render only the message. 404s skip logging.

### Multipart (`src/extractors/multipart.rs`)

`Multipart<F>` deserializes a multipart form into any `F: DeserializeOwned`. It uses `helpers::struct_fields::<F>()` (a fake serde `Deserializer` that only captures the field name list) to know which fields to accept, streams file parts to disk under `Config::upload_path`, and represents them as `UploadedFile`. It requires the `Config` extension to be present.

### Config (`src/config.rs`)

`Config::from_env` reads `UPLOAD_PATH`, `STATIC_PATH`, `BASE_URL`, `APP_NAME`, and optional `UPLOAD_ROUTE` (default `/uploaded`). `tracing::setup_tracing` reads `LOG_LEVEL` and logs to stdout in debug builds, to a daily rolling file in release.

### Test helpers (`src/tests/helpers.rs`)

Exported for downstream crates, not used internally: `RouterExt::req` runs a request through a `Router` via `oneshot` and returns a `TestResponse` with `contains_str` style assertions. `serial_test` is a dev-dependency for the same purpose.
