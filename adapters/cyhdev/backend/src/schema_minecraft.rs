//! Minecraft declarations extracted from the host Diesel schema; included by its schema module.

diesel::table! {
    minecraft_map_world (minecraft_map_world_id) {
        minecraft_map_world_id -> Text,
    }
}

diesel::table! {
    minecraft_waypoint (minecraft_waypoint_id) {
        minecraft_waypoint_id -> Uuid,
        minecraft_waypoint_world_id -> Text,
        minecraft_waypoint_slot -> Int2,
        minecraft_waypoint_name -> Text,
        minecraft_waypoint_description -> Text,
        minecraft_waypoint_x -> Int4,
        minecraft_waypoint_y -> Int2,
        minecraft_waypoint_z -> Int4,
    }
}
