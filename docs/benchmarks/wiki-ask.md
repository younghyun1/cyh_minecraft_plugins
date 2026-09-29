# WikiAsk measurements

Measured September 29, 2026 on the Linux development host using the checked-in nightly toolchain, Rust edition 2024, development builds, Tantivy 0.26.2, and Codex CLI 0.159.0. These observations are not production latency guarantees. No Minecraft server was launched or contacted.

## Complete documentation snapshot

The public English Minecraft Wiki API supplied 78,047 current pages across content and transclusion namespaces in 1,569 independently compressed batches. The completed index contains 196,699 passages. Talk/user pages, media binaries, and revision history are outside this documentation snapshot. Raw wikitext and Lua are retained; templates are not rendered.

| Data | Bytes | MiB |
| --- | ---: | ---: |
| Complete UTF-8 revision text | 159,440,759 | 152.05 |
| Zstandard archive batches | 40,398,551 | 38.53 |
| Immutable search index | 92,473,242 | 88.19 |

The largest page contains 505,609 bytes. `stats --corpus <snapshot> --index <index>` reproduces these measurements. The snapshot, index, and credentials remain outside the repository.

## Resident loading and local retrieval

An end-to-end development run loaded the corpus/index and initialized Codex IPC in 1,365 ms. Immediately after readiness, Linux reported bridge RSS and high-water RSS of 327,652 KiB, or 319.97 MiB. This excludes the separate Codex and code-mode processes. Budget 512 MiB for the bridge with this snapshot and measure those child processes separately for deployment. The enforced text/index/page bounds and transient overhead calculation are in the [architecture document](../architecture/be/wiki-ask.md).

Each query below ran 100 times against one resident index through `search --repeat 100`. Timing begins after index loading. Values include ranked search and stored-passage extraction; they exclude IPC, model inference, and Java delivery.

| Query | First lookup, µs | Median, µs | p95, µs | First passage |
| --- | ---: | ---: | ---: | --- |
| How do I craft a bucket? | 1,537 | 1,062 | 1,076 | Bucket: Crafting |
| What level is best for diamonds? | 1,582 | 1,170 | 1,185 | Diamond: Mining |
| How do villagers breed? | 1,593 | 1,124 | 1,144 | Villager: Breeding |
| How many bookshelves for level 30? | 1,565 | 1,192 | 1,232 | Bookshelf: Enchanting |

These four checks establish useful sample relevance, not comprehensive retrieval accuracy. The exact article/redirect boost and moderate title weighting prevent unrelated heading matches from dominating these examples. Full literal/regex searches and revision windows remain available for missing evidence.

## IPC and output verification

An explicit real-account test used synthetic Alice/Bob identities and two synthetic world UUIDs with GPT-6 Luna, low reasoning, and fast tier. A bucket question completed in 2,694 ms, a second player's shared-context follow-up in 1,038 ms, and the isolated-world follow-up in 2,126 ms. Bucket crafting coordinates matched the source; Bob recalled Alice's fictional mine name in the same world, and the other world did not. All replies were single paragraphs within 60 words and 360 characters.

A separate real CLI test used a synthetic template absent from ranked results. Luna called local regex search and revision read, then returned the verification marker embedded beyond the initial excerpt. Exactly two fallback calls occurred. This also verified that the code-mode host must remain enabled for Luna to see the dynamic tools.

The final workspace run passed 59 default Rust tests; five explicitly opt-in fixture/timing/inference tests stayed ignored. Workspace Clippy passed with warnings denied. Java compiled with `--release 25`, all lint warnings denied, against existing Paper 26.2 build 123 libraries; ten synthetic bridge checks passed and the development jar was packaged. The existing map UI check also passed TypeScript, 113 tests, and its build. `cargo upgrade --incompatible` completed; every maintained crate uses edition 2024. No Rust or Java check establishes compatibility with an untested live server configuration.

Synthetic tests cover resident search after source-directory removal, redirect ranking, mismatched/incomplete manifests, Unicode page windows, malformed and oversized regex queries, lookup budgets, cursor bounds, oversized/symlinked index entries, speaker/world isolation, final-only event filtering, process reuse after capacity rejection, and permanent shutdown. The [companion README](../../tools/wiki-assistant/README.md) documents reproduction commands and explicit live-test prerequisites.
