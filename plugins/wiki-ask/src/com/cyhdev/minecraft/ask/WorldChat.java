package com.cyhdev.minecraft.ask;

import java.util.UUID;
import net.kyori.adventure.text.Component;
import net.kyori.adventure.text.format.NamedTextColor;
import org.bukkit.entity.Player;

/** Shared conversation delivery stays inside its world and permission boundary. */
final class WorldChat {
    private WorldChat() {}

    static Component question(String username, String question) {
        return Component.text("[Ask] ", NamedTextColor.AQUA)
                .append(Component.text(username + ": ", NamedTextColor.GRAY))
                .append(Component.text(question, NamedTextColor.WHITE));
    }

    /** Called only on the server thread, for both questions and final replies. */
    static void send(Iterable<? extends Player> players, UUID world, Component message) {
        for (Player player : players) {
            if (player.isOnline() && player.hasPermission("wikiask.use")
                    && player.getWorld().getUID().equals(world)) player.sendMessage(message);
        }
    }
}
