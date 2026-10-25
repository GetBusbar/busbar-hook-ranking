<!-- fleet:header:begin (rendered by `busbar-release plugin heal` from GetBusbar/busbar-release template/ and busbar's plugins.yaml; edit it there) -->
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

Configured under the `ranking` module name. The settings document has one key:

```json
{"policy": "cheapest"}
```

| key | type | meaning |
|---|---|---|
| `policy` | string, required | One of `cheapest` (lowest `cost_per_mtok`), `fastest` (lowest measured latency), `least_busy` (most free concurrency), `usage` (most rate headroom). A candidate without the signal is ranked last; when no candidate has it, the hook abstains and the default weighted order applies. |

`open` refuses, in words, a document that is not a JSON object, a missing or non-string
`policy`, a word that is not one of the four, and any other key. An empty document is read as
`{}` and refused as a missing `policy`.

`configure` (a settings push) uses the same grammar, commit-on-ack: a push without `policy`
acknowledges and changes nothing; a valid `policy` becomes the live strategy for the next
`decide`; anything the grammar refuses is a NACK, the live strategy is unchanged, and the reason
is written to the plugin's own log.

`status` and `describe` are not supported and answer `{}`.

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
