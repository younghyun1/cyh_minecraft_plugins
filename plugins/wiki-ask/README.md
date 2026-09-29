# WikiAsk

Paper plugin exposing `/ask <question>`. Answers are private, one terse paragraph, at most 60 words and 360 Unicode characters. Only the final answer appears in chat, followed by clickable wiki attribution. A temporary action-bar indicator shows that the request is running. Reasoning, tool events, partial text, and process output are never sent to chat.

Each Bukkit world UUID owns one shared conversation. Every message includes the world UUID/name and the sender's player UUID/current username. Players in that world share prior questions and answers with distinct speaker annotations; other worlds use separate conversations. The local Rust companion keeps the wiki index and one Codex app-server process warm, communicating over inherited pipes. There is one ranked local search and one model turn per question, using GPT-6 Luna, low reasoning, and fast service tier. See the [companion README](../../tools/wiki-assistant/README.md) for corpus coverage, protocol, retention, and resource bounds.

Targets Paper 26.2+ and Java 25 using public Bukkit/Adventure APIs. Compiled locally against Paper 26.2 build 123; Folia is unsupported. This plugin does not depend on squaremap or the website adapter.

## Development build

Supply an existing compatible Paper installation's library directory through `ASK_LIBRARIES`. These Bash commands only compile, run synthetic boundary tests, and package the plugin. They do not download or launch a Minecraft server.

```bash
ASK_LIBRARIES=/absolute/path/to/paper/libraries
ASK_CLASSPATH=''
while IFS= read -r dependency; do ASK_CLASSPATH="$ASK_CLASSPATH:$dependency"; done < <(rg --files "$ASK_LIBRARIES" -g '*.jar')
mkdir -p plugins/wiki-ask/target/classes
javac --release 25 -Xlint:all,-classfile -Werror -cp "$ASK_CLASSPATH" -d plugins/wiki-ask/target/classes plugins/wiki-ask/src/com/cyhdev/minecraft/ask/*.java plugins/wiki-ask/src/test/com/cyhdev/minecraft/ask/*.java
java -cp "plugins/wiki-ask/target/classes:$ASK_CLASSPATH" com.cyhdev.minecraft.ask.BridgeClientTest
jar --create --file plugins/wiki-ask/target/wiki-ask.jar -C plugins/wiki-ask/target/classes com/cyhdev/minecraft/ask/BridgeClient.class -C plugins/wiki-ask/target/classes com/cyhdev/minecraft/ask/WikiAsk.class -C plugins/wiki-ask plugin.yml -C plugins/wiki-ask config.yml
```

Paper supplies Adventure and Gson; neither is bundled. Java 25 or newer must be available as `javac`, `java`, and `jar`. Rust tests do not verify Paper runtime compatibility.

## Operator setup

Build the companion and acquire a complete wiki snapshot/index using its README. On the server machine, prepare a dedicated directory for Codex authentication and a separate empty working directory, owned by the Minecraft OS user with mode `0700`. Authenticate the dedicated home interactively with `CODEX_HOME=/absolute/ask-codex-home /absolute/codex login`. Use Codex CLI 0.159.0 or a version verified against the documented protocol. Do not add personal config, MCP servers, plugins, skills, hooks, or repository files to that home or work directory.

Set the five absolute paths in `plugins/WikiAsk/config.yml`:

```yaml
bridge-binary: '/absolute/bin/minecraft-wiki-assistant'
wiki-index: '/absolute/wiki/index-2026-09-29'
codex-binary: '/absolute/bin/codex'
codex-home: '/absolute/ask-codex-home'
work-directory: '/absolute/ask-work'
```

Blank or invalid paths disable the plugin. The index must already be complete; no downloader runs on the game server's tick thread. The dedicated Codex login must have access to the requested model and fast tier; a rejected model request fails visibly without switching models. Actual model latency depends on account/service availability.

Copying the reviewed jar and configuring/activating it are separate deployment actions requiring explicit scope. Building this project does not install the plugin, change a live server, or authorize a restart. No live server was used by the synthetic tests.

## Permissions and lifecycle

`wikiask.use` defaults to everyone; server administrators can revoke it through their permission system. Permissions are checked before admission and again before delivering a reply. Questions accept at most 240 characters. Each player has a ten-second cooldown; at most one question runs globally and excess requests receive a short busy response. The worker has no backlog. Cooldown records are removed on disconnect and capped at 4,096 entries.

All Bukkit lookups and chat sends run on the main server thread. Indexing, retrieval, model I/O, and subprocess waits run off-thread. A disconnect or world change drops the response but does not reset the world's conversation. Up to 32 world conversations remain available for the process lifetime without idle expiry or eviction. Additional worlds receive a capacity response; existing conversations remain intact. A bridge restart resets the ephemeral conversations. Auto-compaction is enabled with a 24,000-token threshold and a 32,768-token context setting. Plugin disable kills only its child process tree and shuts down its executors. It never signals a Minecraft service or shared Codex process.

The player/world identity metadata and question are sent to the configured Codex account. Replies remain private to the caller, but their content becomes shared conversation context for subsequent callers in the same world. No world-content, player-state, seed, inventory, location, server-log, or website account integration exists. Questions cannot execute server commands. The bridge does not log question/answer text, and child diagnostics are discarded; normal server command logging is controlled by the server itself.
