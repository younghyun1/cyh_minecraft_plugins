-- Preserve public annotations unless they have been deliberately removed first.
DO $$ BEGIN
    IF EXISTS (SELECT 1 FROM minecraft_waypoint) THEN
        RAISE EXCEPTION 'Remove Minecraft waypoints before rolling back their storage';
    END IF;
END $$;
DROP TABLE minecraft_waypoint;
DROP TABLE minecraft_map_world;
