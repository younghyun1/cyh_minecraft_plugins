package com.cyhdev.minecraft;

import java.io.ByteArrayInputStream;
import java.io.DataInputStream;
import java.io.IOException;
import java.util.ArrayList;
import java.util.List;

/** Selective NBT reader; unrelated chunk payloads are skipped without allocating their object trees. */
final class SavedStructureNbt {
    private static final int MAX_TAGS = 100_000;
    private static final int MAX_DEPTH = 64;
    private final DataInputStream input;
    private int tags;

    private SavedStructureNbt(byte[] bytes) {
        input = new DataInputStream(new ByteArrayInputStream(bytes));
    }

    static List<WorldProtocol.Structure> read(byte[] bytes, int chunkX, int chunkZ) throws IOException {
        var reader = new SavedStructureNbt(bytes);
        if (reader.input.readUnsignedByte() != 10) throw new IOException("Saved chunk root is not a compound");
        reader.name();
        Integer actualX = null;
        Integer actualZ = null;
        var structures = new ArrayList<WorldProtocol.Structure>();
        for (int type; (type = reader.tag(0)) != 0;) {
            String name = reader.name();
            if (name.equals("structures") && type == 10) reader.structures(structures, 1);
            else if (name.equals("xPos") && type == 3) actualX = reader.input.readInt();
            else if (name.equals("zPos") && type == 3) actualZ = reader.input.readInt();
            else reader.skip(type, 1);
        }
        if (actualX == null || actualZ == null || actualX != chunkX || actualZ != chunkZ) {
            throw new IOException("Saved chunk coordinates do not match region entry");
        }
        if (reader.input.available() != 0) throw new IOException("Trailing saved chunk data");
        return List.copyOf(structures);
    }

    private void structures(List<WorldProtocol.Structure> result, int depth) throws IOException {
        for (int type; (type = tag(depth)) != 0;) {
            String name = name();
            if (name.equals("starts") && type == 10) {
                for (int startType; (startType = tag(depth + 1)) != 0;) {
                    name();
                    if (startType == 10) {
                        WorldProtocol.Structure structure = start(depth + 2);
                        if (structure != null) {
                            if (result.size() >= 256) throw new IOException("Too many saved structure starts");
                            result.add(structure);
                        }
                    } else skip(startType, depth + 2);
                }
            } else skip(type, depth + 1);
        }
    }

    private WorldProtocol.Structure start(int depth) throws IOException {
        String kind = null;
        var bounds = new Bounds();
        for (int type; (type = tag(depth)) != 0;) {
            String name = name();
            if (name.equals("id") && type == 8) kind = name();
            else if (name.equals("BB") && type == 11) bounds.include(box());
            else if (name.equals("Children") && type == 9) children(bounds, depth + 1);
            else skip(type, depth + 1);
        }
        if (kind == null || kind.equals("INVALID")) return null;
        if (kind.length() > 128 || !kind.matches("[a-z0-9_.-]+:[a-z0-9_./-]+")) {
            throw new IOException("Invalid saved structure identifier");
        }
        return bounds.value == null ? null : new WorldProtocol.Structure(kind,
                bounds.value[0], bounds.value[1], bounds.value[2],
                bounds.value[3], bounds.value[4], bounds.value[5]);
    }

    private void children(Bounds bounds, int depth) throws IOException {
        int type = input.readUnsignedByte();
        int count = count();
        if (count > 4096 || (count > 0 && type != 10)) throw new IOException("Invalid structure pieces");
        for (int i = 0; i < count; i++) {
            for (int field; (field = tag(depth)) != 0;) {
                String name = name();
                if (name.equals("BB") && field == 11) bounds.include(box());
                else skip(field, depth + 1);
            }
        }
    }

    private int[] box() throws IOException {
        if (input.readInt() != 6) throw new IOException("Invalid structure bounding box");
        int[] value = new int[6];
        for (int i = 0; i < value.length; i++) value[i] = input.readInt();
        for (int axis = 0; axis < 3; axis++) {
            int minimum = axis == 1 ? -2032 : -30_000_000;
            int maximum = axis == 1 ? 2031 : 30_000_000;
            if (value[axis] > value[axis + 3] || value[axis] < minimum || value[axis + 3] > maximum) {
                throw new IOException("Saved structure bounds outside supported world");
            }
        }
        return value;
    }

    private int tag(int depth) throws IOException {
        if (depth > MAX_DEPTH || ++tags > MAX_TAGS) throw new IOException("Saved NBT complexity limit exceeded");
        int type = input.readUnsignedByte();
        if (type > 12) throw new IOException("Unknown saved NBT tag");
        return type;
    }

    private String name() throws IOException {
        input.mark(2);
        int length = input.readUnsignedShort();
        input.reset();
        if (length > 512) throw new IOException("Saved NBT name exceeds limit");
        return input.readUTF();
    }

    private int count() throws IOException {
        int count = input.readInt();
        if (count < 0 || count > 8 * 1024 * 1024) throw new IOException("Invalid saved NBT collection length");
        return count;
    }

    private void skip(int type, int depth) throws IOException {
        if (depth > MAX_DEPTH || ++tags > MAX_TAGS) throw new IOException("Saved NBT complexity limit exceeded");
        switch (type) {
            case 1 -> input.skipNBytes(1);
            case 2 -> input.skipNBytes(2);
            case 3, 5 -> input.skipNBytes(4);
            case 4, 6 -> input.skipNBytes(8);
            case 7 -> input.skipNBytes(count());
            case 8 -> input.skipNBytes(input.readUnsignedShort());
            case 9 -> {
                int element = input.readUnsignedByte();
                int count = count();
                if (element > 12 || (element == 0 && count != 0) || count > MAX_TAGS - tags) {
                    throw new IOException("Invalid saved NBT list");
                }
                for (int i = 0; i < count; i++) skip(element, depth + 1);
            }
            case 10 -> {
                for (int child; (child = tag(depth)) != 0;) {
                    name();
                    skip(child, depth + 1);
                }
            }
            case 11 -> input.skipNBytes((long) count() * 4);
            case 12 -> input.skipNBytes((long) count() * 8);
            default -> throw new IOException("Invalid saved NBT payload type");
        }
    }

    private static final class Bounds {
        private int[] value;

        void include(int[] next) {
            if (value == null) value = next;
            else {
                for (int axis = 0; axis < 3; axis++) {
                    value[axis] = Math.min(value[axis], next[axis]);
                    value[axis + 3] = Math.max(value[axis + 3], next[axis + 3]);
                }
            }
        }
    }
}
