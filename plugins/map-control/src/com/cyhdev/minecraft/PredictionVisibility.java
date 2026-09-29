package com.cyhdev.minecraft;

import java.lang.reflect.Field;
import java.util.ArrayList;
import java.util.List;
import net.minecraft.world.level.border.BorderStatus;
import net.minecraft.world.level.border.WorldBorder;
import xyz.jpenilla.squaremap.common.visibilitylimit.VisibilityShape;

/** Version-locked translation of squaremap's union of visible integer block columns. */
final class PredictionVisibility {
    private static final String SHAPE_PACKAGE = "xyz.jpenilla.squaremap.common.visibilitylimit.";
    private static final int COORDINATE_LIMIT = 30_000_000;
    private static final int MAX_SHAPES = 64;

    private PredictionVisibility() {}

    record Policy(WorldProtocol.Bounds worldBorder, List<WorldProtocol.VisibilityShape> shapes) {}

    static Policy capture(List<VisibilityShape> shapes, WorldBorder border) throws PredictionContext.Unsupported {
        try {
            if (shapes.size() > MAX_SHAPES || border.getStatus() != BorderStatus.STATIONARY) throw unsupported();
            WorldProtocol.Bounds worldBorder = borderBounds(border.getMinX(), border.getMinZ(), border.getMaxX(), border.getMaxZ());
            List<WorldProtocol.VisibilityShape> converted = new ArrayList<>(Math.min(shapes.size(), MAX_SHAPES));
            for (VisibilityShape shape : shapes) {
                if (converted.size() >= MAX_SHAPES) throw unsupported();
                if (shape.getClass().getClassLoader() != VisibilityShape.class.getClassLoader()) throw unsupported();
                String type = shape.getClass().getName();
                if (type.equals(SHAPE_PACKAGE + "RectangleShape")) {
                    converted.add(rectangle(integer(shape, "minBlockX"), integer(shape, "minBlockZ"),
                        integer(shape, "maxBlockX"), integer(shape, "maxBlockZ")));
                } else if (type.equals(SHAPE_PACKAGE + "CircleShape")) {
                    int radius = integer(shape, "radius");
                    // This squaremap version stores radius squared in an int; larger values overflow.
                    if (radius < 1 || radius > 46_340 || integer(shape, "radiusSquared") != radius * radius) throw unsupported();
                    converted.add(circle(integer(shape, "centerX"), integer(shape, "centerZ"), radius));
                } else if (type.equals(SHAPE_PACKAGE + "WorldBorderShape")) {
                    converted.add(squaremapBorder(border.getCenterX(), border.getCenterZ(), border.getSize()));
                } else throw unsupported();
            }
            return new Policy(worldBorder, List.copyOf(converted));
        } catch (ReflectiveOperationException | RuntimeException | LinkageError error) { throw unsupported(); }
    }

    static WorldProtocol.Bounds borderBounds(double minX, double minZ, double maxX, double maxZ) throws PredictionContext.Unsupported {
        finiteCoordinate(minX); finiteCoordinate(minZ); finiteCoordinate(maxX); finiteCoordinate(maxZ);
        if (minX >= maxX || minZ >= maxZ) throw unsupported();
        return bounds((int) Math.ceil(minX), (int) Math.ceil(minZ), (int) Math.ceil(maxX) - 1, (int) Math.ceil(maxZ) - 1);
    }

    static WorldProtocol.RectangleVisibility squaremapBorder(double centerX, double centerZ, double size) throws PredictionContext.Unsupported {
        finiteCoordinate(centerX); finiteCoordinate(centerZ);
        if (!Double.isFinite(size) || size <= 0 || size > COORDINATE_LIMIT * 2.0) throw unsupported();
        int radius = (int) Math.ceil(size / 2);
        // WorldBorderShape truncates its center toward zero and uses an exclusive upper edge.
        return rectangle((int) centerX - radius, (int) centerZ - radius, (int) centerX + radius - 1, (int) centerZ + radius - 1);
    }

    static WorldProtocol.RectangleVisibility rectangle(int minX, int minZ, int maxX, int maxZ) throws PredictionContext.Unsupported {
        bounds(minX, minZ, maxX, maxZ);
        return new WorldProtocol.RectangleVisibility("rectangle", minX, minZ, maxX, maxZ);
    }

    static WorldProtocol.CircleVisibility circle(int centerX, int centerZ, int radius) throws PredictionContext.Unsupported {
        coordinate(centerX); coordinate(centerZ);
        if (radius < 1 || radius > 46_340) throw unsupported();
        return new WorldProtocol.CircleVisibility("circle", centerX, centerZ, radius);
    }

    private static WorldProtocol.Bounds bounds(int minX, int minZ, int maxX, int maxZ) throws PredictionContext.Unsupported {
        coordinate(minX); coordinate(minZ); coordinate(maxX); coordinate(maxZ);
        if (minX > maxX || minZ > maxZ) throw unsupported();
        return new WorldProtocol.Bounds(minX, minZ, maxX, maxZ);
    }

    private static int integer(Object shape, String name) throws ReflectiveOperationException {
        Field field = shape.getClass().getDeclaredField(name);
        if (field.getType() != int.class || !field.trySetAccessible()) throw new IllegalAccessException("Unsupported visibility field");
        return field.getInt(shape);
    }

    private static void finiteCoordinate(double value) throws PredictionContext.Unsupported {
        if (!Double.isFinite(value) || Math.abs(value) > COORDINATE_LIMIT) throw unsupported();
    }

    private static void coordinate(int value) throws PredictionContext.Unsupported {
        if (value < -COORDINATE_LIMIT || value > COORDINATE_LIMIT) throw unsupported();
    }

    private static PredictionContext.Unsupported unsupported() { return new PredictionContext.Unsupported(); }
}
