<!-- fleet:header:begin (rendered by `cargo xtask fleet render` from GetBusbar/busbar's plugins.yaml; edit it there) -->
# busbar-hook-ranking

First-party signed kind:hook plugin cdylib: the ranking hook, packaged as a droppable busbar plugin. Drop the signed tarball into plugins/.

| kind | alias | crate | busbar | license |
|---|---|---|---|---|
| `hook` | `ranking` | `busbar-hook-ranking-plugin` | 1.6.0 (pinned in `.busbar-ref`) | Apache-2.0 |

[![ci](https://github.com/GetBusbar/busbar-hook-ranking/actions/workflows/ci.yml/badge.svg?branch=dev)](https://github.com/GetBusbar/busbar-hook-ranking/actions/workflows/ci.yml)
<!-- fleet:header:end -->

## What it is for

`busbar-hook-ranking` is a `kind: hook` busbar plugin.

## Config

Configured under the `ranking` module name.

## Build

```bash
cargo build --release -p busbar-hook-ranking-plugin
```

## Tests

```bash
cargo test --workspace --locked
```

## License

Apache-2.0. See [LICENSE](LICENSE).
