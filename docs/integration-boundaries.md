# Integration boundaries

The map core owns bounded world queries, seed profiles, tile generation, cache budgets, squaremap file caching and transport validation. The website adapter owns login/session state, database-current administrator authorization, HTTP response envelopes, origin guards, request deadlines and dependency composition. Extracting a crate must preserve these checks; public markers remain readable while mutations require administrator authority.

The current waypoint adapter uses the website's PostgreSQL pool and schema. Its migration and authority tests belong with that adapter. Generic map code should depend on repository and authorization capabilities without importing the website's complete account service.

The Solid/Leaflet map owns rendering, prediction tiles, hover, selection and controls. The website supplies typed transports, administrator state, translations, page shell and shared CSS tokens. Generated HTTP contracts and the binary tile codec must be versioned with both producers and consumers.

Squaremap file serving currently has a separate bounded cache in the website. That cache, loopback management WebSocket transport and private map-control Unix-socket transport must be retained when consolidating the map integration. The seed prediction budget remains separate and capped at 512 MiB.

Deployments supply `SQUAREMAP_WEB_DIR`, `MINECRAFT_WORLD_SOCKET`, `MINECRAFT_MAP_CONTROL_SOCKET`, `MINECRAFT_MANAGEMENT_PORT` and `MINECRAFT_MANAGEMENT_SECRET` through private configuration. `MINECRAFT_SEED_WORKER` is a compatibility setting for the old preview endpoint. Never check values into source control. Unix sockets retain their same-OS-user and restrictive permission requirements.

Building or copying source never authorizes a Minecraft restart. Plugin activation occurs only on a separately authorized manual restart. Ship reviewed deployment examples rather than production OpenRC configuration or server.properties.

WikiAsk is independent of the map and website. Its Paper command passes a bounded question, player UUID/username, and world UUID/name through an inherited child-process pipe. A local Rust companion performs one ranked lookup in an immutable wiki index, then continues that world's shared Codex conversation through a persistent app-server pipe. Only the final short answer returns privately to the caller. All players in a world share prior conversation context, with distinct speaker annotations; other worlds use separate conversations. No HTTP endpoint, world-content/seed/player-state reads, server command execution, or website authority is involved. Codex authentication, snapshots, and index directories are operator-owned external state. The plugin and companion READMEs define admission, session retention, deadlines, provenance, and separate activation steps.
