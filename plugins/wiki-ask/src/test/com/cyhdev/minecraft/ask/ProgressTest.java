package com.cyhdev.minecraft.ask;

import com.google.gson.JsonObject;
import com.google.gson.JsonParser;
import java.io.BufferedReader;
import java.io.IOException;
import java.io.InputStreamReader;
import java.nio.file.Files;
import java.nio.file.Path;
import java.util.ArrayList;
import java.util.Arrays;
import java.util.List;

/** Exercises real bounded pipes with synthetic lifecycle events and unsafe payloads. */
public final class ProgressTest {
    private ProgressTest() {}

    public static void main(String[] args) throws IOException {
        if (args.length > 0) { fixture(args[0]); return; }
        Path work = Files.createTempDirectory("wiki-progress-test-");
        String classpath = String.join(java.io.File.pathSeparator, Arrays.stream(System.getProperty("java.class.path")
                .split(java.io.File.pathSeparator)).map(p -> Path.of(p).toAbsolutePath().toString()).toList());
        try {
            for (String mode : List.of("valid", "unknown", "overflow", "wrong-id", "content")) {
                var phases = new ArrayList<String>();
                List<String> command = List.of(Path.of(System.getProperty("java.home"), "bin", "java").toString(),
                        "-cp", classpath, ProgressTest.class.getName(), mode, work.toString());
                try (BridgeClient client = new BridgeClient(command, phases::add)) {
                    try {
                        String[] response = client.ask("player", "Alice", "world", "Survival", "question");
                        check(mode.equals("valid"));
                        check(response[0].contains("timed out while compacting"));
                        check(response[1].isEmpty());
                        check(!response[0].contains("SECRET"));
                        check(phases.equals(List.of("starting", "answering", "compacting", "resetting")));
                    } catch (IOException expected) { check(!mode.equals("valid")); }
                }
            }
            RequestStatus status = new RequestStatus();
            status.begin("ask");
            status.update("compacting");
            check(status.busy().contains("compacting conversation history"));
            check(status.busy().contains("not queued"));
            status.update("resetting");
            check(status.disconnected(true).contains("timed out while compacting"));
            check(!RequestStatus.failure("SECRET", "answering").contains("SECRET"));
        } finally { Files.deleteIfExists(work); }
        System.out.println("Progress: 13 checks passed");
    }

    private static void fixture(String mode) throws IOException {
        System.out.println("{\"ready\":true,\"protocol\":2}");
        var reader = new BufferedReader(new InputStreamReader(System.in, java.nio.charset.StandardCharsets.UTF_8));
        JsonObject request = JsonParser.parseString(BridgeClient.readLine(reader)).getAsJsonObject();
        long id = request.get("id").getAsLong();
        List<String> phases = switch (mode) {
            case "valid" -> List.of("answering", "compacting", "resetting");
            case "overflow" -> java.util.Collections.nCopies(33, "answering");
            case "unknown" -> List.of("SECRET");
            default -> List.of("answering");
        };
        for (String phase : phases) {
            JsonObject frame = new JsonObject();
            frame.addProperty("id", mode.equals("wrong-id") ? id + 1 : id);
            frame.addProperty("status", phase);
            if (mode.equals("content")) frame.addProperty("reasoning", "SECRET");
            System.out.println(frame);
        }
        JsonObject failure = new JsonObject();
        failure.addProperty("id", id);
        failure.addProperty("error_code", "timeout");
        failure.addProperty("fatal", true);
        failure.addProperty("error", "SECRET provider response");
        System.out.println(failure);
    }

    private static void check(boolean condition) {
        if (!condition) throw new AssertionError("Progress boundary assertion failed");
    }
}
