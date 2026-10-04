package com.cyhdev.minecraft.ask;

import java.io.IOException;
import java.util.concurrent.TimeUnit;

/** Fixed public wording; subprocess and provider text never becomes a status message. */
final class RequestStatus {
    private record State(String phase, String operation, long started) {}
    private volatile State state = new State("starting", "starting", System.nanoTime());

    void begin(String action) {
        String phase = switch (action) { case "clear" -> "clearing"; case "compact" -> "compacting"; default -> "connecting"; };
        state = new State(phase, phase, System.nanoTime());
    }

    void update(String phase) {
        State previous = state;
        state = new State(phase, phase.equals("resetting") ? previous.operation() : phase, previous.started());
    }

    static String description(String phase) throws IOException {
        return switch (phase) {
            case "starting" -> "Loading the local wiki and starting Codex";
            case "connecting" -> "Opening a world conversation in Codex";
            case "answering" -> "Codex is answering a Minecraft question";
            case "searching" -> "Searching the local Minecraft wiki";
            case "compacting" -> "Codex is compacting conversation history";
            case "clearing" -> "Clearing a world's conversation";
            case "resetting" -> "Resetting a failed Codex connection";
            default -> throw new IOException("Unknown bridge status");
        };
    }

    String progress() {
        State current = state;
        long seconds = TimeUnit.NANOSECONDS.toSeconds(System.nanoTime() - current.started());
        try { return description(current.phase()) + " (" + seconds + "s elapsed)."; }
        catch (IOException invalid) { return "WikiAsk received an invalid status."; }
    }

    String busy() { return progress() + " Your command was not queued; retry when it finishes."; }

    String disconnected(boolean timedOut) {
        return failure(timedOut ? "timeout" : "connection", state.operation());
    }

    static String failure(String code, String phase) {
        String operation = switch (phase) {
            case "compacting" -> "compacting conversation history";
            case "clearing" -> "clearing the conversation";
            case "starting", "connecting" -> "starting the conversation";
            default -> "answering the question";
        };
        return switch (code) {
            case "timeout" -> "Codex timed out while " + operation + ". Connection reset; try /ask again.";
            case "connection" -> "The Codex connection closed while " + operation + ". It will restart on the next /ask.";
            case "rate_limit" -> "Codex reached its usage or rate limit. Try again later.";
            case "access" -> "Codex authentication or model access failed. A server operator must check the login.";
            case "service_unavailable" -> "Codex could not reach its service. Try again shortly.";
            case "response" -> "Codex failed to return a valid reply. Connection reset; try /ask again.";
            case "local_data" -> "The local wiki or Codex configuration failed. A server operator must check the logs.";
            case "capacity" -> "Minecraft help has reached its 32-world conversation limit.";
            default -> "WikiAsk received an invalid failure response. A server operator must check the logs.";
        };
    }
}
