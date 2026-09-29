package dev.startingequipment;

import org.bukkit.plugin.java.JavaPlugin;

public class StartingEquipment extends JavaPlugin {

    @Override
    public void onEnable() {
        getServer().getPluginManager().registerEvents(new JoinListener(this), this);
        getLogger().info("StartingEquipment enabled.");
    }
}
