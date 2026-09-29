# Minecraft Wiki assistant

Rust edition 2024 companion for [WikiAsk](../../plugins/wiki-ask/README.md). It downloads the English Minecraft Wiki's current documentation, stores complete revision text in independent Zstandard-compressed batches, and builds an immutable Tantivy section index. Startup decompresses all revision text and copies the index into RAM. Each question gets one ranked lookup before one Codex turn; that turn can request up to three bounded wiki lookups or page reads when the initial evidence is insufficient. No embedding service, public wiki requests, or shell search runs on the chat path.

## Prepare a snapshot

Run from the workspace root with the checked-in nightly toolchain. Use paths outside the repository for downloaded material and credentials.

```sh
cargo build --locked --package minecraft-wiki-assistant
target/debug/minecraft-wiki-assistant download --corpus /absolute/wiki/corpus-2026-09-29
target/debug/minecraft-wiki-assistant index --corpus /absolute/wiki/corpus-2026-09-29 --index /absolute/wiki/index-2026-09-29
target/debug/minecraft-wiki-assistant stats --corpus /absolute/wiki/corpus-2026-09-29 --index /absolute/wiki/index-2026-09-29
target/debug/minecraft-wiki-assistant search --index /absolute/wiki/index-2026-09-29 --repeat 100 'How do I craft a bucket?'
```

The importer discovers every namespace marked as content by the wiki, plus Project, Template, Help, Category, and Module. This includes tutorials and the other Minecraft games. It downloads all current pages and redirects in those namespaces, with page IDs, revision IDs, timestamps, and complete main-slot text. Talk pages, user profiles, media binaries, and historical revisions are excluded. This is a documentation snapshot, not a backup of the wiki installation. Wikitext and Lua transclusion source are preserved; the index does not execute Lua or render templates. Answers must acknowledge gaps in the retrieved evidence.

Requests are serial, spaced by 500 ms, use `maxlag=5`, identify this project, and bound HTTP responses and serialized batches to 32 MiB. Throttling and server overload receive at most five retries with bounded backoff. A checkpoint after each batch supports rerunning the same download command. A local file lock prevents concurrent importers. Compressed storage is capped at 8 GiB; crossing a limit fails visibly. A failed download is never marked complete. The source website can change during a crawl; revision URLs identify each exact downloaded version.

The index command requires a complete manifest and creates a new directory. Failed indexes lack `wiki-manifest.json` and cannot be served. Remove only that failed output directory before retrying, or select a new path. Refresh by downloading into a new corpus directory and building a new index. Point the plugin at that completed immutable index during a separately authorized deployment. No scheduled refresh or production mutation is implicit.

## Retrieval and IPC

Sections are divided into 1,800-character passages with 200-character overlap. English stemming, literal tokens, BM25, a 1.5-fold title boost, and exact article/redirect matching produce at most six passages, with at most two per page. Simple explicit crafting grids receive a row/column annotation alongside the original source. Player text cannot become query operators or wildcard expansions. A short follow-up also uses the prior question for local retrieval. Prior messages and answers stay in the world's shared Codex conversation regardless of the retrieval query.

The September 29 snapshot contains 78,047 revisions: 152.05 MiB of UTF-8 text, 38.53 MiB compressed, and an 88.19 MiB index with 196,699 passages. Both text and index remain resident; the serving path has no corpus mmap or disk lookup dependency. Loading rejects more than 100,000 pages, 256 MiB of retained text, 16 MiB of title/timestamp allocations, or 128 MiB of index files. The corpus and index manifests must match. Allow additional memory for page records, revision maps, Tantivy readers, eight decompressed store-cache blocks per segment, transient batch decoding, allocator overhead, and the separate Codex/code-mode processes. These are data bounds, not an RSS guarantee; see [measurements](../../docs/benchmarks/wiki-ask.md). They are independent of the map prediction and squaremap budgets. The offline writer uses one worker and a 64 MiB indexing budget.

Request-time ranked retrieval admits at most 24 distinct tokens, 24 candidate hits, six stored passages, and 10,800 passage characters. `wiki_search` also offers case-insensitive literal and Rust-regex scans over complete resident titles and text, including templates, redirects, and modules. Queries are capped at 160 characters, regex compilation at 1 MiB, and the DFA cache at 4 MiB. A scan stops after eight excerpts or 250 ms between pages and returns a continuation cursor when incomplete. Excerpts contain at most 700 characters. `wiki_read` returns up to 6,000 characters from a revision ID and Unicode offset, with a continuation offset. Tool output is capped at 48 KiB and three calls per answer, within the same 15-second turn deadline. These tools accept no commands or filesystem paths.

The runtime session map is capped at 32 worlds; new worlds are rejected at capacity without evicting existing context. Codex uses a 32,768-token context setting and auto-compacts at 24,000 tokens instead of resetting long active conversations. Sessions have no idle expiry and survive player reconnects, but are ephemeral and reset on bridge restart.

The plugin starts this process off-thread during enable with `serve --corpus ... --index ... --codex ... --codex-home ... --work-dir ...`. This process starts `codex app-server --stdio` once. Both links use private inherited pipes, not TCP or a shared Codex daemon. Every question carries the player's UUID/current username and the world's UUID/name. The world UUID selects the shared conversation; user-entered text cannot select another world's context.

Only completed `agentMessage` output explicitly marked `final_answer` is eligible for a reply. Reasoning events, deltas, tools, intermediate explanations, and child stderr never enter the reply pipe. Both Rust and Java enforce a single paragraph of at most 60 words and 360 Unicode characters, with control/formatting characters removed. The plugin logs no questions or answers. Minecraft itself may log slash commands according to its own server settings.

The Codex process explicitly selects `gpt-6-luna`, `model_reasoning_effort="low"`, and `service_tier="fast"`; provider model fallback is disabled. Luna requires the isolated code-mode host to expose the two dynamic wiki tools, so that host stays enabled. Shell, browser, computer, image, app, plugin, hook, and multi-agent features are disabled, environment access is empty, and the sandbox is read-only. Unexpected approval requests fail closed; unknown dynamic calls return a bounded error. Its dedicated home must contain no user configuration, skills, plugins, or hooks, and its working directory must be empty. The plugin sends the question, player/world identity metadata, retrieved public passages, and retained shared conversation to Codex; it does not collect world contents, seed, inventory, location, server-log, or permission data.

## Protocol

All frames are newline-delimited JSON. Startup emits `{"ready":true,"protocol":1}` after resident loading and IPC initialization. Requests are limited to 4,096 UTF-8 bytes and contain exactly `id`, `player_uuid`, `username`, `world_uuid`, `world_name`, and `question`. Replies echo `id` and contain either `answer`, `sources`, `tool_calls`, `retrieval_micros`, and `elapsed_ms`, or a generic `error`. No reasoning field exists. One request may be in flight. The CLI initialization deadline is 20 seconds, session acquisition has five seconds, inference has 15 seconds, and Java has a 43-second outer watchdog. A world-capacity rejection preserves the process and existing conversations. Transport/model failure ends the child, never replays a charged request, and allows a new process on the next query.

## Verification

```sh
cargo test --locked --package minecraft-wiki-assistant
cargo clippy --locked --package minecraft-wiki-assistant --all-targets -- -D warnings
```

Default tests use synthetic pages, an actual temporary Tantivy index, and a duplex JSON-RPC fixture. They never authenticate, contact a model, read server logs, or start Minecraft. An explicit opt-in integration test exercises the locally installed Codex CLI with synthetic conversation text:

```sh
WIKI_TEST_CODEX=/absolute/path/to/codex WIKI_TEST_AUTH_FILE=/absolute/dedicated/auth.json cargo test --locked --package minecraft-wiki-assistant live_cli_keeps_prior_messages -- --ignored
```

It creates a temporary isolated home with a temporary link to the supplied auth file, verifies cross-player memory and a real regex-search/page-read fallback against a synthetic marker, and removes the temporary home afterwards. The end-to-end `live_bridge_shares_world_context_and_isolates_other_worlds` test additionally requires `WIKI_TEST_BRIDGE`, `WIKI_TEST_CORPUS`, and `WIKI_TEST_INDEX`; it starts only the companion and Codex, verifies real wiki retrieval, and checks world isolation. `search --repeat` reports retrieval timing separately from process startup and model latency. It does not claim an inference latency SLA.

## Sources and licensing

Downloaded content belongs to Minecraft Wiki contributors. The API currently reports [CC BY-NC-SA 3.0](https://creativecommons.org/licenses/by-nc-sa/3.0/); each snapshot preserves the reported license and every revision's attribution URL. The chat includes a clickable Wiki attribution with the license and an indication that the text is summarized. Keep the manifest with any redistributed snapshot/index and respect its noncommercial/share-alike terms. The corpus and index are external data, not checked-in project source.

The IPC contract was generated and inspected from Codex CLI 0.159.0 using `codex app-server generate-json-schema --experimental`. No generated contract files are vendored. Settings and transport references: [Codex app-server](https://learn.chatgpt.com/docs/app-server), [Codex configuration](https://learn.chatgpt.com/docs/config-file/config-reference), and [Tantivy](https://docs.rs/tantivy/0.26.2/tantivy/).
