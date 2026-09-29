# Minecraft Wiki assistant

Rust companion for [WikiAsk](../../plugins/wiki-ask/README.md). It downloads the English Minecraft Wiki's current documentation, stores complete revision text in independent Zstandard-compressed batches, builds an immutable Tantivy section index, and serves one local lookup followed by one Codex turn per question. No embedding service, wiki requests, shell search, or model-driven retrieval loop runs on the chat path.

## Prepare a snapshot

Run from the workspace root with the checked-in nightly toolchain. Use paths outside the repository for downloaded material and credentials.

```sh
cargo build --locked --package minecraft-wiki-assistant
target/debug/minecraft-wiki-assistant download --corpus /absolute/wiki/corpus-2026-09-29
target/debug/minecraft-wiki-assistant index --corpus /absolute/wiki/corpus-2026-09-29 --index /absolute/wiki/index-2026-09-29
target/debug/minecraft-wiki-assistant search --index /absolute/wiki/index-2026-09-29 --repeat 100 'How do I craft a bucket?'
```

The importer discovers every namespace marked as content by the wiki, plus Project, Template, Help, Category, and Module. This includes tutorials and the other Minecraft games. It downloads all current pages and redirects in those namespaces, with page IDs, revision IDs, timestamps, and complete main-slot text. Talk pages, user profiles, media binaries, and historical revisions are excluded. This is a documentation snapshot, not a backup of the wiki installation. Wikitext and Lua transclusion source are preserved; the index does not execute Lua or render templates. Answers must acknowledge gaps in the retrieved evidence.

Requests are serial, spaced by 500 ms, use `maxlag=5`, identify this project, and bound HTTP responses and serialized batches to 32 MiB. Throttling and server overload receive at most five retries with bounded backoff. A checkpoint after each batch supports rerunning the same download command. A local file lock prevents concurrent importers. Compressed storage is capped at 8 GiB; crossing a limit fails visibly. A failed download is never marked complete. The source website can change during a crawl; revision URLs identify each exact downloaded version.

The index command requires a complete manifest and creates a new directory. Failed indexes lack `wiki-manifest.json` and cannot be served. Remove only that failed output directory before retrying, or select a new path. Refresh by downloading into a new corpus directory and building a new index. Point the plugin at that completed immutable index during a separately authorized deployment. No scheduled refresh or production mutation is implicit.

## Retrieval and IPC

Sections are divided into 1,800-character passages with 200-character overlap. English stemming, literal tokens, BM25, and a fourfold title boost produce at most six passages, with at most two per page. Player text cannot become query operators or wildcard expansions. A short follow-up also uses the prior question for local retrieval. Prior messages and answers stay in the player's Codex conversation regardless of the retrieval query.

The index is memory mapped and opened once. The writer uses one worker and a 64 MiB indexing budget. Request-time retrieval admits at most 24 distinct tokens, 24 candidate hits, six stored passages, and 10,800 passage characters. The only growing runtime session map is capped at 32 players; idle sessions expire after 30 minutes and the least recently used session is evicted at capacity. Codex uses a 32,768-token context setting and compacts at 24,000 tokens instead of resetting long active conversations. Sessions are ephemeral and survive reconnects within retention, but not bridge restarts or eviction. These bounds are independent of the map prediction and squaremap budgets.

The plugin starts this process with `serve --index ... --codex ... --codex-home ... --work-dir ...`. This process starts `codex app-server --stdio` once. Both links use private inherited pipes, not TCP or a shared Codex daemon. Every question carries the Bukkit UUID and current username. The UUID selects the conversation; user-entered text cannot select another player's session.

Only completed `agentMessage` output that is not commentary is eligible for a reply. Reasoning events, deltas, tools, intermediate explanations, and child stderr never enter the reply pipe. Both Rust and Java enforce a single paragraph of at most 60 words and 360 Unicode characters, with control/formatting characters removed. The plugin logs no questions or answers. Minecraft itself may log slash commands according to its own server settings.

The Codex process explicitly selects `gpt-6-luna`, `model_reasoning_effort="low"`, and `service_tier="fast"`; provider model fallback is disabled. Shell, browser, computer, image, app, plugin, hook, and multi-agent features are disabled, environment access is empty, and the sandbox is read-only. Unexpected approval/tool requests fail closed. Its dedicated home must contain no user configuration, skills, plugins, or hooks, and its working directory must be empty. The plugin sends the question, UUID, username, retrieved public passages, and retained conversation to Codex; it does not collect world, seed, inventory, location, server-log, or permission data.

## Protocol

All frames are newline-delimited JSON. Startup emits `{"ready":true,"protocol":1}`. Requests are limited to 4,096 UTF-8 bytes and contain exactly `id`, `player_uuid`, `username`, and `question`. Replies echo `id` and contain either `answer`, `sources`, `retrieval_micros`, and `elapsed_ms`, or a generic `error`. No reasoning field exists. One request may be in flight. The CLI initialization deadline is 20 seconds, session acquisition has five seconds, inference has 15 seconds, and Java has a 43-second outer watchdog. Failure ends the child, never replays a charged request, and allows a new process on the next query.

## Verification

```sh
cargo test --locked --package minecraft-wiki-assistant
cargo clippy --locked --package minecraft-wiki-assistant --all-targets -- -D warnings
```

Default tests use synthetic pages, an actual temporary Tantivy index, and a duplex JSON-RPC fixture. They never authenticate, contact a model, read server logs, or start Minecraft. An explicit opt-in integration test exercises the locally installed Codex CLI with synthetic conversation text:

```sh
WIKI_TEST_CODEX=/absolute/path/to/codex WIKI_TEST_AUTH_FILE=/absolute/dedicated/auth.json cargo test --locked --package minecraft-wiki-assistant live_cli_keeps_prior_messages -- --ignored
```

It creates a temporary isolated home with a temporary link to the supplied auth file, verifies a follow-up remembers the prior message, and removes the temporary home afterwards. `search --repeat` reports retrieval timing separately from process startup and model latency. It does not claim an inference latency SLA.

## Sources and licensing

Downloaded content belongs to Minecraft Wiki contributors. The API currently reports [CC BY-NC-SA 3.0](https://creativecommons.org/licenses/by-nc-sa/3.0/); each snapshot preserves the reported license and every revision's attribution URL. The chat includes a clickable Wiki attribution with the license and an indication that the text is summarized. Keep the manifest with any redistributed snapshot/index and respect its noncommercial/share-alike terms. The corpus and index are external data, not checked-in project source.

The IPC contract was generated and inspected from Codex CLI 0.159.0 using `codex app-server generate-json-schema --experimental`. No generated contract files are vendored. Settings and transport references: [Codex app-server](https://learn.chatgpt.com/docs/app-server), [Codex configuration](https://learn.chatgpt.com/docs/config-file/config-reference), and [Tantivy](https://docs.rs/tantivy/0.26.2/tantivy/).
