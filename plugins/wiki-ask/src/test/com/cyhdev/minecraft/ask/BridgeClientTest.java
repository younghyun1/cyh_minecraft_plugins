package com.cyhdev.minecraft.ask;

import java.io.BufferedReader;
import java.io.IOException;
import java.io.StringReader;
import java.io.InputStreamReader;
import java.nio.charset.StandardCharsets;
import java.nio.file.Files;
import java.nio.file.Path;
import java.util.Arrays;
import java.util.List;
import com.google.gson.JsonObject;
import com.google.gson.JsonParser;

/** Synthetic formatting and pipe-boundary checks; no Minecraft or Codex process. */
public final class BridgeClientTest {
    private BridgeClientTest() {}

    public static void main(String[] args) throws IOException {
        if (args.length > 0 && args[0].equals("--fixture")) { fixture(); return; }
        if (args.length > 0 && args[0].equals("--live")) { live(args); return; }
        if (args.length > 0 && args[0].equals("--startup-failure")) {
            System.out.println("{\"ready\":false,\"protocol\":1,\"diagnostic\":\"dedicated Codex home must not contain custom skills\"}");
            return;
        }
        String text = BridgeClient.plain("§\u202eReason\n" + "🪨 word ".repeat(1000));
        check(!text.contains("\n") && !text.contains("§") && !text.contains("\u202e"));
        check(text.codePointCount(0, text.length()) <= 360);
        check(text.split(" ").length <= 60);
        check(BridgeClient.readLine(new BufferedReader(new StringReader("ok\n"))).equals("ok"));
        for (String invalid : new String[] {"missing newline", "a".repeat(4096) + "\n"}) {
            boolean rejected = false;
            try { BridgeClient.readLine(new BufferedReader(new StringReader(invalid))); }
            catch (IOException expected) { rejected = true; }
            check(rejected);
        }
        Path work = Files.createTempDirectory("wiki-ask-test-");
        String classpath = String.join(java.io.File.pathSeparator, Arrays.stream(System.getProperty("java.class.path")
                .split(java.io.File.pathSeparator)).map(p -> Path.of(p).toAbsolutePath().toString()).toList());
        List<String> command = List.of(Path.of(System.getProperty("java.home"), "bin", "java").toString(),
                "-cp", classpath, BridgeClientTest.class.getName(), "--fixture", work.toString());
        try (BridgeClient client = new BridgeClient(command)) {
            client.prepare();
            client.prepare();
            String[] rejected = client.ask("player-id", "Alice", "world-id", "Survival", "first");
            check(rejected[0].equals("At capacity."));
            String[] reply = client.ask("player-id", "Bob", "world-id", "Survival", "second");
            check(reply[0].equals("Bob in Survival; previous requests: 1"));
            check(reply[1].endsWith("/100"));
            client.shutdown();
            boolean stopped = false;
            try { client.prepare(); } catch (IOException expected) { stopped = true; }
            check(stopped);
        }
        List<String> failedCommand = new java.util.ArrayList<>(command);
        failedCommand.set(failedCommand.size() - 2, "--startup-failure");
        try (BridgeClient client = new BridgeClient(failedCommand)) {
            boolean explained = false;
            try { client.prepare(); }
            catch (IOException expected) { explained = expected.getMessage().contains("custom skills"); }
            check(explained);
        } finally { Files.deleteIfExists(work); }
        System.out.println("BridgeClient: 11 checks passed");
    }

    /** Explicit opt-in verifies the exact Java environment and repeated starts with one Codex home. */
    private static void live(String[] args) throws IOException {
        if (args.length != 7) throw new IOException("Expected --live bridge index corpus codex codex-home work-directory");
        List<String> command = List.of(args[1], "serve", "--index", args[2], "--corpus", args[3],
                "--codex", args[4], "--codex-home", args[5], "--work-dir", args[6]);
        for (int attempt = 0; attempt < 2; attempt++) {
            try (BridgeClient client = new BridgeClient(command)) {
                client.prepare();
                String[] reply = client.ask("12345678-1234-1234-1234-123456789abc", "JavaProbe",
                        "00000000-0000-0000-0000-000000000001", "SyntheticWorld",
                        "How fast is a blue ice boat highway compared to a packed ice highway?");
                check(reply[0].toLowerCase(java.util.Locale.ROOT).contains("ice"));
                check(!reply[0].contains("unavailable"));
                check(reply[0].codePointCount(0, reply[0].length()) <= 360);
                System.out.println("Java bridge start " + (attempt + 1) + ": " + reply[0]);
            }
        }
    }

    private static void fixture() throws IOException {
        System.out.println("{\"ready\":true,\"protocol\":1}");
        var reader = new BufferedReader(new InputStreamReader(System.in, StandardCharsets.UTF_8));
        for (int i = 0; i < 2; i++) {
            JsonObject request = JsonParser.parseString(BridgeClient.readLine(reader)).getAsJsonObject();
            check(request.get("player_uuid").getAsString().equals("player-id"));
            check(request.get("world_uuid").getAsString().equals("world-id"));
            JsonObject response = new JsonObject();
            response.add("id", request.get("id"));
            if (i == 0) response.addProperty("error", "At capacity.");
            else {
                response.addProperty("answer", request.get("username").getAsString() + " in "
                        + request.get("world_name").getAsString() + "; previous requests: " + i);
                response.add("sources", JsonParser.parseString("[\"https://minecraft.wiki/w/Special:Redirect/revision/100\"]"));
            }
            System.out.println(response);
        }
    }

    private static void check(boolean condition) {
        if (!condition) throw new AssertionError("Bridge boundary assertion failed");
    }
}
