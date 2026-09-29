CREATE TABLE minecraft_map_world (
    minecraft_map_world_id text PRIMARY KEY,
    CONSTRAINT minecraft_map_world_key CHECK (char_length(minecraft_map_world_id) <= 128 AND minecraft_map_world_id ~ '^[a-z0-9_.-]+:[a-z0-9_./-]+$')
);

CREATE TABLE minecraft_waypoint (
    minecraft_waypoint_id uuid PRIMARY KEY DEFAULT uuidv7(),
    minecraft_waypoint_world_id text NOT NULL REFERENCES minecraft_map_world (minecraft_map_world_id) ON DELETE RESTRICT,
    minecraft_waypoint_slot smallint NOT NULL,
    minecraft_waypoint_name text NOT NULL,
    minecraft_waypoint_description text NOT NULL,
    minecraft_waypoint_x integer NOT NULL,
    minecraft_waypoint_y smallint NOT NULL,
    minecraft_waypoint_z integer NOT NULL,
    CONSTRAINT minecraft_waypoint_capacity CHECK (minecraft_waypoint_slot BETWEEN 0 AND 255),
    CONSTRAINT minecraft_waypoint_world_slot UNIQUE (minecraft_waypoint_world_id, minecraft_waypoint_slot),
    CONSTRAINT minecraft_waypoint_name_bound CHECK (char_length(minecraft_waypoint_name) BETWEEN 1 AND 80 AND minecraft_waypoint_name ~ '[^[:space:]]' AND minecraft_waypoint_name !~ '[[:cntrl:]]'),
    CONSTRAINT minecraft_waypoint_description_bound CHECK (char_length(minecraft_waypoint_description) <= 500 AND minecraft_waypoint_description !~ '[[:cntrl:]]'),
    CONSTRAINT minecraft_waypoint_x_bound CHECK (minecraft_waypoint_x BETWEEN -30000000 AND 30000000),
    CONSTRAINT minecraft_waypoint_z_bound CHECK (minecraft_waypoint_z BETWEEN -30000000 AND 30000000),
    CONSTRAINT minecraft_waypoint_y_bound CHECK (minecraft_waypoint_y BETWEEN -2032 AND 2031)
);
