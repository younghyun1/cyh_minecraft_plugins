# Persistent spawn chunks

Function-only Minecraft 26.3 datapack, format 121. On first load, it enables forced loading for a 65 by 65 chunk square centered on the Overworld origin: chunk coordinates -32 through 32 on each axis, 4,225 chunks total. The functions issue one 65-chunk row per command so each command stays below the command area limit.

`cyh_spawn:enable` adds those tickets and records enabled state. `cyh_spawn:disable` removes those tickets and records disabled state. `cyh_spawn:load` only initializes an unused state; disabling remains effective across later loads. Forced-load ticket persistence belongs to the world; no ticket or world data is included here.

This is a source snapshot, not an installer. Copying or building the repository does not activate the datapack. Review its fixed footprint and the server's chunk budget before choosing to install it. The repository contains no command that automatically reloads or restarts a server.
