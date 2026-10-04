# WikiAsk

Paper plugin exposing `/ask <question>`. Accepted questions appear as `[Ask] username: question`, followed by the final answer, for online players in the same world with `wikiask.use`. Answers are one terse paragraph, at most 60 words and 360 Unicode characters, followed by clickable wiki attribution. A refreshed action bar shows the current operation and total elapsed time to permitted players in that world. Reasoning, raw tool events, partial text, and process output are never sent to chat.

Each Bukkit world UUID owns one shared conversation. Every message includes the world UUID/name and the sender's player UUID/current username. Players in that world share prior questions and answers with distinct speaker annotations; other worlds use separate conversations. The local Rust companion loads all wiki text and the index into RAM during startup and keeps one Codex app-server process warm, communicating over inherited pipes. There is one ranked local search before each model turn, using GPT-6.1 Sol, medium reasoning, and fast service tier. Up to three literal/regex/ranked lookups or full-page windows are available when more evidence is needed. See the [companion README](../../tools/wiki-assistant/README.md) for corpus coverage, protocol, retention, and resource bounds.

Clearly unrelated questions receive exactly `Question irrelevant to purpose`, without attribution or an explanation. Minecraft mechanics, creative builds, strategy, calculations, speculation, and contextual follow-ups remain allowed. The same turn classifies relevance and provides a structured answer; rejected content is discarded, and malformed classifications fail closed. Semantic classification is model-based, not a keyword guarantee. Adding a Minecraft label to an unrelated task is explicitly insufficient.

Busy responses identify startup, local search, answering, compaction, clearing, or failed-connection cleanup, with total elapsed seconds and an explicit statement that the new command was not queued. Progress is refreshed every two seconds. Failure replies distinguish timeout, connection loss, provider usage limits, authentication/access, service availability, invalid responses, and local configuration problems using fixed wording. Automatic compaction is shown without exposing its summary. Normal answers have a 45-second turn deadline, allowing medium reasoning and automatic compaction, and a 75-second Java watchdog including cold startup; a deadline is not an expected response time.

Targets Paper 26.2+ and Java 25 using public Bukkit/Adventure APIs. Compiled locally against Paper 26.2 build 123; Folia is unsupported. This plugin does not depend on squaremap or the website adapter.

## Conversation commands

`/ask clear` and `/ask compact` require server operator status and `wikiask.use`. Both affect only the caller's current world's shared conversation, use the existing cooldown and global busy gate, and announce completion to permitted players in that world. Non-operators cannot invoke or tab-complete these controls; ordinary questions remain available under `wikiask.use`.

`clear` unsubscribes the old conversation and removes its follow-up retrieval context; the next question creates a fresh conversation. Other worlds remain intact. `compact` asks Codex to summarize the existing conversation, retaining useful context rather than resetting it. Confirmation waits for successful compaction completion, with a 35-second operation deadline and 60-second Java watchdog including cold startup. An unused world reports that there is nothing to compact. Controls use explicit protocol actions and are never submitted as gameplay prompts. Only a single exact `clear` or `compact` argument selects a control; longer questions remain questions.

## Development build

Supply an existing compatible Paper installation's library directory through `ASK_LIBRARIES`. These Bash commands only compile, run synthetic boundary tests, and package the plugin. They do not download or launch a Minecraft server.

```bash
ASK_LIBRARIES=/absolute/path/to/paper/libraries
ASK_CLASSPATH=''
while IFS= read -r dependency; do ASK_CLASSPATH="$ASK_CLASSPATH:$dependency"; done < <(rg --files "$ASK_LIBRARIES" -g '*.jar')
mkdir -p plugins/wiki-ask/target/classes
javac --release 25 -Xlint:all,-classfile -Werror -cp "$ASK_CLASSPATH" -d plugins/wiki-ask/target/classes plugins/wiki-ask/src/com/cyhdev/minecraft/ask/*.java plugins/wiki-ask/src/test/com/cyhdev/minecraft/ask/*.java
java -cp "plugins/wiki-ask/target/classes:$ASK_CLASSPATH" com.cyhdev.minecraft.ask.BridgeClientTest
java -cp "plugins/wiki-ask/target/classes:$ASK_CLASSPATH" com.cyhdev.minecraft.ask.WorldChatTest
java -cp "plugins/wiki-ask/target/classes:$ASK_CLASSPATH" com.cyhdev.minecraft.ask.ProgressTest
jar --create --file plugins/wiki-ask/target/wiki-ask.jar -C plugins/wiki-ask/target/classes com/cyhdev/minecraft/ask/BridgeClient.class -C plugins/wiki-ask/target/classes com/cyhdev/minecraft/ask/WikiAsk.class -C plugins/wiki-ask/target/classes com/cyhdev/minecraft/ask/WorldChat.class -C plugins/wiki-ask/target/classes com/cyhdev/minecraft/ask/RequestStatus.class -C plugins/wiki-ask/target/classes 'com/cyhdev/minecraft/ask/RequestStatus$State.class' -C plugins/wiki-ask plugin.yml -C plugins/wiki-ask config.yml
```

Paper supplies Adventure and Gson; neither is bundled. Java 25 or newer must be available as `javac`, `java`, and `jar`. Rust tests do not verify Paper runtime compatibility.

## Operator setup

Build the companion and acquire a complete wiki snapshot/index using its README. On the server machine, prepare a dedicated directory for Codex authentication and a separate empty working directory, owned by the Minecraft OS user with mode `0700`. Authenticate the dedicated home interactively with `CODEX_HOME=/absolute/ask-codex-home /absolute/codex login`. Use Codex CLI 0.159.0 or a version verified against the documented protocol. Do not add personal config, MCP servers, plugins, custom skills, hooks, or repository files to that home or work directory. Codex creates bundled files in `skills/.system` on first launch; that directory is accepted on subsequent starts without enabling skill discovery.

Set the six absolute paths in `plugins/WikiAsk/config.yml`:

```yaml
bridge-binary: '/absolute/bin/minecraft-wiki-assistant'
wiki-index: '/absolute/wiki/index-2026-09-29'
wiki-corpus: '/absolute/wiki/corpus-2026-09-29'
codex-binary: '/absolute/bin/codex'
codex-home: '/absolute/ask-codex-home'
work-directory: '/absolute/ask-work'
```

Blank or invalid paths disable the plugin. The corpus and index must already be complete and have matching manifests. Startup decompresses and retains all text and index data on the background worker; commands receive a short busy response until initialization finishes. The September 29 snapshot contains 152 MiB of text plus an 88 MiB index, before runtime overhead; see [memory measurements](../../docs/benchmarks/wiki-ask.md). No downloader runs on the game server's tick thread. The dedicated Codex login must have access to the requested model and fast tier; a rejected model request fails visibly without switching models. Actual model latency depends on account/service availability.

Copying the reviewed jar and configuring/activating it are separate deployment actions requiring explicit scope. Building this project does not install the plugin, change a live server, or authorize a restart. When updating a running Paper server, stage the replacement jar in its configured update directory for the next authorized restart. No live server was used by the synthetic tests.

An explicitly requested live verification can run `BridgeClientTest --live <bridge> <index> <corpus> <codex> <codex-home> <work-directory>` with the same Java classpath. It makes two synthetic gameplay requests using separate companion starts and the same populated Codex home. This checks Java's cleared subprocess environment and restart compatibility without invoking the Minecraft command or changing a world. Startup failures return bounded operator diagnostics to the plugin log; raw child output remains discarded.

The same arguments with `--live-controls` verify that a synthetic remembered detail survives manual compaction and disappears after clearing. This creates its own companion and conversations; it does not control the running plugin's sessions.

`LivePolicyTest <bridge> <index> <corpus> <codex> <codex-home> <work-directory>` explicitly exercises the configured account with synthetic Minecraft reasoning, contextual follow-ups, and unrelated/injection requests. It validates exact rejection text, attribution suppression, final output bounds, and the progress pipe. Protocol 2 requires deploying the jar and companion together.

WikiAsk 0.1.3 passed nine live scope/reply checks and five clear/compact checks with CLI 0.159.2 on Paper 26.3's Java classpath. The five unrelated requests returned the exact rejection in 1.35-1.98 seconds; the four Minecraft requests took 2.27-6.09 seconds. These are deployment observations, not a latency guarantee. Manual compaction retained a synthetic base name and clearing removed it.

## Permissions and lifecycle

`wikiask.use` defaults to everyone; server administrators can revoke it through their permission system. Permissions are checked before admission and again before delivering a reply. Questions accept at most 240 characters. Each player has a ten-second cooldown; at most one question runs globally and excess requests receive a short busy response. The worker has no backlog. Cooldown records are removed on disconnect and capped at 4,096 entries.

All Bukkit lookups and chat sends run on the main server thread. Indexing, retrieval, model I/O, and subprocess waits run off-thread. A disconnect or world change drops the response but does not reset the world's conversation. Up to 32 world conversations remain available for the process lifetime without idle expiry or eviction. Additional worlds receive a capacity response; existing conversations remain intact. A bridge restart resets the ephemeral conversations. Auto-compaction is enabled with a 24,000-token threshold and a 32,768-token context setting. Plugin disable kills only its child process tree and shuts down its executors. It never signals a Minecraft service or shared Codex process.

The player/world identity metadata and question are sent to the configured Codex account. Questions and final answers are shared in that world's chat and conversation context. Recipients must be online, in the originating world, and currently hold `wikiask.use`; rejected commands receive only a private status reply. No world-content, player-state, seed, inventory, location, server-log, or website account integration exists. Questions cannot execute server commands. The bridge does not log question/answer text, and raw child diagnostics are discarded; normal server command logging is controlled by the server itself.
