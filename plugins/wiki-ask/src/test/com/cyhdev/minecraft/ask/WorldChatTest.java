package com.cyhdev.minecraft.ask;

import java.lang.reflect.Proxy;
import java.util.ArrayList;
import java.util.List;
import java.util.UUID;
import net.kyori.adventure.text.Component;
import net.kyori.adventure.text.serializer.plain.PlainTextComponentSerializer;
import org.bukkit.World;
import org.bukkit.entity.Player;

/** Synthetic Bukkit interfaces verify recipients and literal message rendering without a server. */
public final class WorldChatTest {
    private WorldChatTest() {}
    private record Recipient(Player player, List<Component> messages) {}

    public static void main(String[] args) {
        UUID world = UUID.fromString("00000000-0000-0000-0000-000000000001");
        Recipient alice = recipient(world, true, true, true);
        Recipient bob = recipient(world, true, true, false);
        Recipient elsewhere = recipient(UUID.fromString("00000000-0000-0000-0000-000000000002"), true, true, false);
        Recipient denied = recipient(world, true, false, true);
        Recipient offline = recipient(world, false, true, false);
        check(WorldChat.canControl(alice.player()));
        check(!WorldChat.canControl(bob.player()));
        check(!WorldChat.canControl(denied.player()));
        List<Player> players = List.of(alice.player(), bob.player(), elsewhere.player(), denied.player(), offline.player());
        Component question = WorldChat.question("Alice", "Is <red>blue ice</red> faster?");
        Component answer = Component.text("Blue ice is faster.");
        WorldChat.send(players, world, question);
        WorldChat.send(players, world, answer);
        Component progress = Component.text("Compacting conversation history (6s elapsed).");
        WorldChat.actionBar(players, world, progress);
        for (Recipient recipient : List.of(alice, bob)) {
            check(recipient.messages().equals(List.of(question, answer, progress)));
            check(PlainTextComponentSerializer.plainText().serialize(recipient.messages().get(0))
                    .equals("[Ask] Alice: Is <red>blue ice</red> faster?"));
        }
        for (Recipient recipient : List.of(elsewhere, denied, offline)) check(recipient.messages().isEmpty());
        System.out.println("WorldChat: 10 checks passed");
    }

    private static Recipient recipient(UUID id, boolean online, boolean permitted, boolean operator) {
        World world = (World) Proxy.newProxyInstance(World.class.getClassLoader(), new Class<?>[] {World.class},
                (proxy, method, arguments) -> {
                    if (method.getName().equals("getUID")) return id;
                    throw new AssertionError("Unexpected world access: " + method.getName());
                });
        List<Component> messages = new ArrayList<>();
        Player player = (Player) Proxy.newProxyInstance(Player.class.getClassLoader(), new Class<?>[] {Player.class},
                (proxy, method, arguments) -> switch (method.getName()) {
                    case "isOnline" -> online;
                    case "isOp" -> operator;
                    case "getWorld" -> world;
                    case "hasPermission" -> permitted && arguments[0].equals("wikiask.use");
                    case "sendMessage", "sendActionBar" -> { messages.add((Component) arguments[0]); yield null; }
                    default -> throw new AssertionError("Unexpected player access: " + method.getName());
                });
        return new Recipient(player, messages);
    }

    private static void check(boolean condition) {
        if (!condition) throw new AssertionError("World chat boundary assertion failed");
    }
}
