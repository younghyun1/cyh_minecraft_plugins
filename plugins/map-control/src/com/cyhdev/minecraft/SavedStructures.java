package com.cyhdev.minecraft;

import java.io.ByteArrayInputStream;
import java.io.IOException;
import java.io.InputStream;
import java.nio.ByteBuffer;
import java.nio.channels.FileChannel;
import java.nio.file.Files;
import java.nio.file.LinkOption;
import java.nio.file.NoSuchFileException;
import java.nio.file.Path;
import java.nio.file.StandardOpenOption;
import java.util.List;
import java.util.zip.GZIPInputStream;
import java.util.zip.InflaterInputStream;

/** Reads saved structure starts without loading structure-reference chunks in the game server. */
final class SavedStructures {
    private static final int SECTOR_BYTES = 4096;
    private static final int MAX_COMPRESSED = 1024 * 1024;
    private static final int MAX_DECOMPRESSED = 8 * 1024 * 1024;

    record Result(List<WorldProtocol.Structure> structures, boolean truncated) {}

    private SavedStructures() {}

    /** The supplied path is Paper's dimension-specific world path followed by {@code region}. */
    static Result read(Path regionDirectory, int chunkX, int chunkZ) throws IOException {
        Path path = regionDirectory.resolve("r." + Math.floorDiv(chunkX, 32) + "."
                + Math.floorDiv(chunkZ, 32) + ".mca");
        try {
            if (!Files.readAttributes(path, java.nio.file.attribute.BasicFileAttributes.class,
                    LinkOption.NOFOLLOW_LINKS).isRegularFile()) {
                throw new IOException("Region is not a regular file");
            }
            try (FileChannel channel = FileChannel.open(path, StandardOpenOption.READ, LinkOption.NOFOLLOW_LINKS)) {
                int index = Math.floorMod(chunkX, 32) + 32 * Math.floorMod(chunkZ, 32);
                ByteBuffer before = readAt(channel, 0, SECTOR_BYTES * 2);
                int location = before.getInt(index * 4);
                int timestamp = before.getInt(SECTOR_BYTES + index * 4);
                if (location == 0) return new Result(List.of(), true);
                long offset = (long) (location >>> 8) * SECTOR_BYTES;
                int sectors = location & 255;
                if (offset < SECTOR_BYTES * 2 || sectors == 0
                        || offset + (long) sectors * SECTOR_BYTES > channel.size()) {
                    throw new IOException("Invalid region allocation");
                }
                ByteBuffer header = readAt(channel, offset, 5);
                int length = header.getInt();
                int compression = Byte.toUnsignedInt(header.get());
                if (length < 1 || length > MAX_COMPRESSED || length + 4 > sectors * SECTOR_BYTES) {
                    throw new IOException("Invalid saved chunk length");
                }
                // External chunks and LZ4 need a separate bounded decoder. Missing coverage is explicit.
                if ((compression & 128) != 0 || compression < 1 || compression > 3) {
                    return new Result(List.of(), true);
                }
                byte[] encoded = readAt(channel, offset + 5, length - 1).array();
                List<WorldProtocol.Structure> structures;
                try (InputStream decoded = decompress(encoded, compression)) {
                    byte[] nbt = decoded.readNBytes(MAX_DECOMPRESSED + 1);
                    if (nbt.length > MAX_DECOMPRESSED) throw new IOException("Saved chunk expands beyond limit");
                    structures = SavedStructureNbt.read(nbt, chunkX, chunkZ);
                }
                ByteBuffer after = readAt(channel, 0, SECTOR_BYTES * 2);
                if (after.getInt(index * 4) != location
                        || after.getInt(SECTOR_BYTES + index * 4) != timestamp) {
                    return new Result(List.of(), true);
                }
                return new Result(structures, false);
            }
        } catch (NoSuchFileException missing) {
            // Newly generated chunks may not have reached the region file yet.
            return new Result(List.of(), true);
        }
    }

    private static InputStream decompress(byte[] bytes, int compression) throws IOException {
        var input = new ByteArrayInputStream(bytes);
        return switch (compression) {
            case 1 -> new GZIPInputStream(input);
            case 2 -> new InflaterInputStream(input);
            case 3 -> input;
            default -> throw new IOException("Unsupported saved chunk compression");
        };
    }

    private static ByteBuffer readAt(FileChannel channel, long offset, int size) throws IOException {
        ByteBuffer buffer = ByteBuffer.allocate(size);
        while (buffer.hasRemaining()) {
            int count = channel.read(buffer, offset + buffer.position());
            if (count < 0) throw new IOException("Truncated region file");
            if (count == 0) throw new IOException("Region file read made no progress");
        }
        return buffer.flip();
    }
}
