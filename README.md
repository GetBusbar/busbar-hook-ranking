<!-- fleet:header:begin (rendered by `busbar-release plugin heal` from GetBusbar/busbar-release template/ and busbar's plugins.yaml; edit it there) -->
# busbar-hook-ranking

First-party signed kind:hook plugin cdylib: the ranking hook, packaged as a droppable busbar plugin. Drop the signed tarball into plugins/.

| kind | alias | crate | busbar | license |
|---|---|---|---|---|
| `hook` | `ranking` | `busbar-hook-ranking-plugin` | 1.6.0 (pinned in `.busbar-ref`) | Apache-2.0 |

[![ci](https://github.com/GetBusbar/busbar-hook-ranking/actions/workflows/ci.yml/badge.svg?branch=dev)](https://github.com/GetBusbar/busbar-hook-ranking/actions/workflows/ci.yml)
<!-- fleet:header:end -->

## What it is for

Ranks candidate routes by cost, measured latency, free concurrency or rate headroom. Each decision
is synchronous and reads only the supplied routing signals; it performs no I/O and reads no prompt
or caller identity.

## Config

Configured under the `ranking` module name. The settings document requires `policy` and optionally supplies `expected_units`:

```json
{"policy": "cheapest", "expected_units": {"work": 10}}
```

| key | type | meaning |
|---|---|---|
| `policy` | string, required | One of `cheapest` (lowest estimated total cost), `fastest` (lowest measured latency), `least_busy` (most free concurrency), `usage` (most rate headroom). A candidate without the signal is ranked last; when no candidate has it, the hook abstains and the engine's default SWRR order applies. |
| `expected_units` | object, optional | Maps non-empty opaque billable class names to finite non-negative expected quantities. Each named value replaces that class's plane-declared default for `cheapest`; omitted classes keep their defaults. |

`open` refuses, in words, a document that is not a JSON object, a missing or non-string
`policy`, a word that is not one of the four, an invalid `expected_units` map, and any other key. An empty document is read as
`{}` and refused as a missing `policy`.

`configure` (a settings push) validates the settings before comparing versions. A valid version
older than the last applied version is acknowledged without applying it. An invalid push is a
NACK even when stale. Equal or newer versions commit on ack: a push without `policy` acknowledges and
retains the strategy; an omitted `expected_units` retains the profile, while an empty object
restores plane defaults. Strategy and profile commit together for the next `decide`. Anything the
grammar refuses is a NACK, the live strategy and profile are unchanged, and the reason is written to the
plugin's own log under `BUSBAR-7150`. The refusal returned to the host is in bare words; the host
names the hook. The SDK refuses a non-object push before this hook runs; that refusal does not
emit the plugin's `BUSBAR-7150` diagnostic.

Each accepted push returns a complete configuration checkpoint through the SDK, including the
retained strategy and expected-units profile. The host uses that checkpoint when recovering a
quarantined instance, so partial or empty pushes preserve the effective configuration after recovery.

For a generic cost projection, `cheapest` sums each recipe term's card-priced routing micro-units
per unit times its selected expected quantity. Configured floating prices retain their original
precision for routing; exact integer billing is unchanged. Duplicate class terms remain separate and each uses
the override. Empty, malformed, overflowed or unknown-price estimates sort last; an override of
zero does not turn an unknown price into zero. An explicitly known zero price remains valid.
A missing projection is unknown, even when every candidate lacks one and carries a
`cost_per_mtok` scalar. Only the current recipe supplies routing cost; missing data never switches
to a different cost model or bypasses expected-unit overrides. These estimates do not alter billing
or the rate card.

Ties use ascending candidate `idx`, independent of input order. A `NaN` signal sorts after
numbers and before absent signals. Missing signals stay reachable at the end; `fastest` does
not probe unsampled candidates, and `usage` preserves the same absent-signal rule when no rate
limit applies. `weighted` belongs to the engine's default order and is refused as a plugin policy.

`status` and `describe` are not supported and answer `{}`.

## Build

```bash
cargo build --release -p busbar-hook-ranking-plugin --features dropped-in
```

## Tests

The dropped-in tests (`door_witness`, `configure_witness`, `conformance`) load the freshly built
cdylib named by `BUSBAR_PLUGIN_CDYLIB`, an absolute path; they fail, never skip, when it is unset or
missing. Build the image with its door, then run the suite, as CI does:

```bash
cargo build --locked -p busbar-hook-ranking-plugin --lib --features dropped-in
# libbusbar_hook_ranking_plugin.dylib on macOS
export BUSBAR_PLUGIN_CDYLIB="$PWD/target/debug/libbusbar_hook_ranking_plugin.so"
cargo test --workspace --locked
```

## License

Apache-2.0. See [LICENSE](LICENSE).
