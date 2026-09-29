package com.cyhdev.minecraft;

import java.io.ByteArrayOutputStream;
import java.io.DataOutputStream;
import java.io.IOException;
import java.io.OutputStream;
import java.nio.ByteBuffer;
import java.nio.file.Files;
import java.nio.file.Path;
import java.util.Arrays;
import java.util.zip.GZIPOutputStream;
import java.util.zip.DeflaterOutputStream;

/** Standalone fixtures exercise region framing and NBT parsing without loading Bukkit or a world. */
public final class SavedStructuresTest {
    private SavedStructuresTest() {}

    public static void main(String[] args) throws IOException {
        Path directory = Files.createTempDirectory("cyhdev-structures-");
        try {
            byte[] nbt = fixture(-1, -33);
            for (int compression = 1; compression <= 3; compression++) {
                writeRegion(directory, -1, -33, compress(nbt, compression), compression);
                var result = SavedStructures.read(directory, -1, -33);
                check(!result.truncated() && result.structures().size() == 1, "saved start was not decoded");
                var structure = result.structures().getFirst();
                check(structure.kind().equals("minecraft:village_plains"), "structure identity changed");
                check(structure.min_x() == -20 && structure.min_y() == 60 && structure.min_z() == -530,
                        "minimum bounds did not combine structure pieces");
                check(structure.max_x() == -3 && structure.max_y() == 82 && structure.max_z() == -510,
                        "maximum bounds did not combine structure pieces");
            }
            check(SavedStructures.read(directory, 0, 0).truncated(), "missing save must report incomplete coverage");
            check(SavedStructures.read(directory, -2, -33).truncated(), "missing chunk slot must report incomplete coverage");

            writeRegion(directory, -1, -33, new byte[0], 4);
            check(SavedStructures.read(directory, -1, -33).truncated(), "LZ4 must not appear as an empty complete result");
            writeRegion(directory, -1, -33, new byte[0], 130);
            check(SavedStructures.read(directory, -1, -33).truncated(), "external chunks must report incomplete coverage");

            expectFailure(() -> SavedStructureNbt.read(nbt, 1, 1), "wrong chunk coordinates");
            expectFailure(() -> SavedStructureNbt.read(fixture(-1, -33, 2032), -1, -33), "unsupported structure height");
            expectFailure(() -> SavedStructureNbt.read(Arrays.copyOf(nbt, nbt.length - 1), -1, -33), "truncated compound");
            byte[] trailing = Arrays.copyOf(nbt, nbt.length + 1);
            expectFailure(() -> SavedStructureNbt.read(trailing, -1, -33), "trailing NBT data");
            expectFailure(() -> SavedStructureNbt.read(nestedFixture(), 0, 0), "excessive nesting");
            expectFailure(() -> SavedStructureNbt.read(malformedList(), 0, 0), "negative list length");

            byte[] expansion = new byte[8 * 1024 * 1024 + 1];
            writeRegion(directory, -1, -33, compress(expansion, 2), 2);
            expectFailure(() -> SavedStructures.read(directory, -1, -33), "excessive decompression");
            Path region = directory.resolve("r.-1.-2.mca");
            Files.write(region, new byte[12]);
            expectFailure(() -> SavedStructures.read(directory, -1, -33), "truncated region header");
            byte[] invalidAllocation = new byte[8192];
            ByteBuffer.wrap(invalidAllocation).putInt((31 + 31 * 32) * 4, (1 << 8) | 1);
            Files.write(region, invalidAllocation);
            expectFailure(() -> SavedStructures.read(directory, -1, -33), "allocation overlaps header");
            System.out.println("SavedStructuresTest: region compression, coordinates, bounds, coverage, and corruption checks passed");
        } finally {
            try (var files = Files.list(directory)) {
                for (Path file : files.toList()) Files.deleteIfExists(file);
            }
            Files.deleteIfExists(directory);
        }
    }

    private static byte[] fixture(int chunkX, int chunkZ) throws IOException {
        return fixture(chunkX, chunkZ, 82);
    }

    private static byte[] fixture(int chunkX, int chunkZ, int maximumY) throws IOException {
        var bytes = new ByteArrayOutputStream();
        try (var out = new DataOutputStream(bytes)) {
            named(out, 10, "");
            integer(out, "xPos", chunkX);
            integer(out, "zPos", chunkZ);
            named(out, 12, "ignored_longs");
            out.writeInt(2);
            out.writeLong(42);
            out.writeLong(100);
            named(out, 10, "structures");
            named(out, 10, "References");
            named(out, 12, "minecraft:other_structure");
            out.writeInt(1);
            out.writeLong(4);
            out.writeByte(0);
            named(out, 10, "starts");
            named(out, 10, "minecraft:village_plains");
            named(out, 8, "id");
            out.writeUTF("minecraft:village_plains");
            named(out, 9, "Children");
            out.writeByte(10);
            out.writeInt(2);
            box(out, new int[] {-20, 63, -530, -10, 75, -520});
            out.writeByte(0);
            box(out, new int[] {-15, 60, -525, -3, maximumY, -510});
            out.writeByte(0);
            out.writeByte(0);
            named(out, 10, "minecraft:empty");
            named(out, 8, "id");
            out.writeUTF("INVALID");
            out.writeByte(0);
            out.writeByte(0);
            out.writeByte(0);
            out.writeByte(0);
        }
        return bytes.toByteArray();
    }

    private static byte[] nestedFixture() throws IOException {
        var bytes = new ByteArrayOutputStream();
        try (var out = new DataOutputStream(bytes)) {
            named(out, 10, "");
            for (int i = 0; i < 70; i++) named(out, 10, "nested");
            for (int i = 0; i < 71; i++) out.writeByte(0);
        }
        return bytes.toByteArray();
    }

    private static byte[] malformedList() throws IOException {
        var bytes = new ByteArrayOutputStream();
        try (var out = new DataOutputStream(bytes)) {
            named(out, 10, "");
            named(out, 9, "bad_list");
            out.writeByte(1);
            out.writeInt(-1);
            out.writeByte(0);
        }
        return bytes.toByteArray();
    }

    private static void named(DataOutputStream out, int type, String name) throws IOException {
        out.writeByte(type);
        out.writeUTF(name);
    }

    private static void integer(DataOutputStream out, String name, int value) throws IOException {
        named(out, 3, name);
        out.writeInt(value);
    }

    private static void box(DataOutputStream out, int[] value) throws IOException {
        named(out, 11, "BB");
        out.writeInt(value.length);
        for (int coordinate : value) out.writeInt(coordinate);
    }

    private static byte[] compress(byte[] bytes, int compression) throws IOException {
        var result = new ByteArrayOutputStream();
        try (OutputStream output = switch (compression) {
            case 1 -> new GZIPOutputStream(result);
            case 2 -> new DeflaterOutputStream(result);
            default -> result;
        }) {
            output.write(bytes);
        }
        return result.toByteArray();
    }

    private static void writeRegion(Path directory, int x, int z, byte[] payload, int compression) throws IOException {
        int sectors = (payload.length + 5 + 4095) / 4096;
        var buffer = ByteBuffer.allocate((2 + sectors) * 4096);
        int slot = Math.floorMod(x, 32) + 32 * Math.floorMod(z, 32);
        buffer.putInt(slot * 4, (2 << 8) | sectors);
        buffer.putInt(4096 + slot * 4, 123);
        buffer.position(8192);
        buffer.putInt(payload.length + 1);
        buffer.put((byte) compression);
        buffer.put(payload);
        Files.write(directory.resolve("r." + Math.floorDiv(x, 32) + "." + Math.floorDiv(z, 32) + ".mca"), buffer.array());
    }

    private static void check(boolean condition, String message) {
        if (!condition) throw new AssertionError(message);
    }

    private static void expectFailure(IoAction action, String label) throws IOException {
        try { action.run(); }
        catch (IOException expected) { return; }
        throw new AssertionError("Accepted " + label);
    }

    @FunctionalInterface
    private interface IoAction { void run() throws IOException; }
}
