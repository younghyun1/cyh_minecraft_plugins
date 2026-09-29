package dev.startingequipment;

import java.io.IOException;
import java.nio.file.Files;
import java.nio.file.Path;
import java.nio.file.StandardOpenOption;
import java.util.HashMap;
import java.util.HashSet;
import java.util.Map;
import java.util.Set;
import java.util.UUID;
import net.kyori.adventure.text.Component;
import org.bukkit.Material;
import org.bukkit.NamespacedKey;
import org.bukkit.World;
import org.bukkit.enchantments.Enchantment;
import org.bukkit.entity.Player;
import org.bukkit.event.EventHandler;
import org.bukkit.event.Listener;
import org.bukkit.event.player.PlayerChangedWorldEvent;
import org.bukkit.event.player.PlayerJoinEvent;
import org.bukkit.inventory.ItemStack;
import org.bukkit.inventory.PlayerInventory;

public class JoinListener implements Listener {

    private static final String GIVEN_FILE_NAME =
        "starting_equipment_given.txt";

    private final StartingEquipment plugin;
    private final Map<String, Set<UUID>> givenPlayersPerWorld = new HashMap<>();

    public JoinListener(StartingEquipment plugin) {
        this.plugin = plugin;
    }

    private Path dimensionDataFile(World world) {
        return world.getWorldFolder().toPath().resolve(GIVEN_FILE_NAME);
    }

    private String namespaceWorldFolderName(World world) {
        String worldName = world.getName();
        if (
            world.getEnvironment() == World.Environment.NETHER &&
            worldName.endsWith("_nether")
        ) {
            return worldName.substring(
                0,
                worldName.length() - "_nether".length()
            );
        }
        if (
            world.getEnvironment() == World.Environment.THE_END &&
            worldName.endsWith("_the_end")
        ) {
            return worldName.substring(
                0,
                worldName.length() - "_the_end".length()
            );
        }
        return worldName;
    }

    private Path namespaceWorldFolder(World world) {
        Path currentWorldFolder = world.getWorldFolder().toPath();
        Path parent = currentWorldFolder.getParent();
        if (parent == null) {
            return currentWorldFolder;
        }

        Path candidate = parent.resolve(namespaceWorldFolderName(world));
        if (Files.isDirectory(candidate)) {
            return candidate;
        }
        return currentWorldFolder;
    }

    private Path dataFileForWorld(World world) {
        return namespaceWorldFolder(world).resolve(GIVEN_FILE_NAME);
    }

    private Set<Path> legacyDimensionDataFiles(World world) {
        Set<Path> legacyFiles = new HashSet<>();
        Path currentWorldFolder = world.getWorldFolder().toPath();
        Path parent = currentWorldFolder.getParent();
        if (parent == null) {
            legacyFiles.add(dimensionDataFile(world));
            return legacyFiles;
        }

        String baseWorldName = namespaceWorldFolderName(world);
        legacyFiles.add(parent.resolve(baseWorldName).resolve(GIVEN_FILE_NAME));
        legacyFiles.add(
            parent.resolve(baseWorldName + "_nether").resolve(GIVEN_FILE_NAME)
        );
        legacyFiles.add(
            parent.resolve(baseWorldName + "_the_end").resolve(GIVEN_FILE_NAME)
        );
        legacyFiles.add(dimensionDataFile(world));
        return legacyFiles;
    }

    private String namespaceWorldKey(World world) {
        NamespacedKey worldKey = world.getKey();
        return worldKey.getNamespace() + ":" + namespaceWorldFolderName(world);
    }

    private void loadPlayersFromFile(Path dataFile, Set<UUID> loaded) {
        if (!Files.exists(dataFile)) {
            return;
        }

        try {
            int lineNumber = 0;
            for (String line : Files.readAllLines(dataFile)) {
                lineNumber++;
                String trimmed = line.trim();
                if (trimmed.isEmpty()) {
                    continue;
                }
                try {
                    loaded.add(UUID.fromString(trimmed));
                } catch (IllegalArgumentException e) {
                    plugin
                        .getLogger()
                        .warning(
                            "Skipping invalid UUID in " +
                                dataFile +
                                " at line " +
                                lineNumber +
                                ": " +
                                trimmed
                        );
                }
            }
        } catch (IOException e) {
            plugin
                .getLogger()
                .warning("Failed to load " + dataFile + ": " + e.getMessage());
        }
    }

    private Set<UUID> getGivenPlayers(World world) {
        String worldKey = namespaceWorldKey(world);
        if (givenPlayersPerWorld.containsKey(worldKey)) {
            return givenPlayersPerWorld.get(worldKey);
        }

        Set<UUID> loaded = new HashSet<>();
        Path namespaceDataFile = dataFileForWorld(world);
        loadPlayersFromFile(namespaceDataFile, loaded);

        for (Path legacyDimensionFile : legacyDimensionDataFiles(world)) {
            if (!legacyDimensionFile.equals(namespaceDataFile)) {
                loadPlayersFromFile(legacyDimensionFile, loaded);
            }
        }

        givenPlayersPerWorld.put(worldKey, loaded);
        return loaded;
    }

    private boolean markPlayer(World world, UUID uuid) {
        Set<UUID> given = getGivenPlayers(world);
        if (!given.add(uuid)) {
            return true;
        }

        Path dataFile = dataFileForWorld(world);
        try {
            Files.createDirectories(dataFile.getParent());
            Files.writeString(
                dataFile,
                uuid + "\n",
                StandardOpenOption.CREATE,
                StandardOpenOption.APPEND
            );
            return true;
        } catch (IOException e) {
            given.remove(uuid);
            plugin
                .getLogger()
                .warning("Failed to save " + dataFile + ": " + e.getMessage());
            return false;
        }
    }

    private void checkAndGiveEquipment(Player player, World world) {
        String worldKey = namespaceWorldKey(world);

        // Check if player has already received equipment in this world.
        Set<UUID> givenPlayers = getGivenPlayers(world);
        if (givenPlayers.contains(player.getUniqueId())) {
            return; // Already received, do nothing.
        }

        // Not yet received. Mark them and give equipment.
        if (!markPlayer(world, player.getUniqueId())) {
            return;
        }
        giveEquipment(player);
        plugin
            .getLogger()
            .info(
                "Gave starting equipment to " +
                    player.getName() +
                    " in namespace world " +
                    worldKey
            );
    }

    @EventHandler
    public void onJoin(PlayerJoinEvent event) {
        checkAndGiveEquipment(event.getPlayer(), event.getPlayer().getWorld());
    }

    @EventHandler
    public void onWorldChange(PlayerChangedWorldEvent event) {
        checkAndGiveEquipment(event.getPlayer(), event.getPlayer().getWorld());
    }

    private void giveEquipment(Player player) {
        PlayerInventory inv = player.getInventory();
        String name = player.getName();

        // === ARMOR ===
        // Helmet
        ItemStack helmet = new ItemStack(Material.NETHERITE_HELMET);
        helmet.addUnsafeEnchantment(Enchantment.BLAST_PROTECTION, 4);
        helmet.addUnsafeEnchantment(Enchantment.UNBREAKING, 3);
        helmet.addUnsafeEnchantment(Enchantment.MENDING, 1);
        helmet.addUnsafeEnchantment(Enchantment.AQUA_AFFINITY, 1);
        helmet.addUnsafeEnchantment(Enchantment.RESPIRATION, 3);
        setName(helmet, name + "'s Helmet");
        inv.setHelmet(helmet);

        // Chestplate
        ItemStack chestplate = new ItemStack(Material.NETHERITE_CHESTPLATE);
        chestplate.addUnsafeEnchantment(Enchantment.PROTECTION, 4);
        chestplate.addUnsafeEnchantment(Enchantment.UNBREAKING, 3);
        chestplate.addUnsafeEnchantment(Enchantment.MENDING, 1);
        setName(chestplate, name + "'s Chestplate");
        inv.setChestplate(chestplate);

        // Leggings
        ItemStack leggings = new ItemStack(Material.NETHERITE_LEGGINGS);
        leggings.addUnsafeEnchantment(Enchantment.PROTECTION, 4);
        leggings.addUnsafeEnchantment(Enchantment.UNBREAKING, 3);
        leggings.addUnsafeEnchantment(Enchantment.MENDING, 1);
        leggings.addUnsafeEnchantment(Enchantment.SWIFT_SNEAK, 3);
        setName(leggings, name + "'s Leggings");
        inv.setLeggings(leggings);

        // Boots — depth strider (faster swimming), no frost walker
        ItemStack boots = new ItemStack(Material.NETHERITE_BOOTS);
        boots.addUnsafeEnchantment(Enchantment.FIRE_PROTECTION, 4);
        boots.addUnsafeEnchantment(Enchantment.UNBREAKING, 3);
        boots.addUnsafeEnchantment(Enchantment.MENDING, 1);
        boots.addUnsafeEnchantment(Enchantment.DEPTH_STRIDER, 3);
        boots.addUnsafeEnchantment(Enchantment.FEATHER_FALLING, 4);
        boots.addUnsafeEnchantment(Enchantment.SOUL_SPEED, 3);
        setName(boots, name + "'s Boots");
        inv.setBoots(boots);

        // === TOOLS ===
        // Sword
        ItemStack sword = new ItemStack(Material.NETHERITE_SWORD);
        sword.addUnsafeEnchantment(Enchantment.SHARPNESS, 5);
        sword.addUnsafeEnchantment(Enchantment.UNBREAKING, 3);
        sword.addUnsafeEnchantment(Enchantment.MENDING, 1);
        sword.addUnsafeEnchantment(Enchantment.LOOTING, 3);
        sword.addUnsafeEnchantment(Enchantment.SWEEPING_EDGE, 3);
        setName(sword, name + "'s Sword");
        inv.addItem(sword);

        // Pickaxe — Silk Touch
        ItemStack pickSilk = new ItemStack(Material.NETHERITE_PICKAXE);
        pickSilk.addUnsafeEnchantment(Enchantment.EFFICIENCY, 5);
        pickSilk.addUnsafeEnchantment(Enchantment.UNBREAKING, 3);
        pickSilk.addUnsafeEnchantment(Enchantment.MENDING, 1);
        pickSilk.addUnsafeEnchantment(Enchantment.SILK_TOUCH, 1);
        setName(pickSilk, name + "'s Silk Touch Pickaxe");
        inv.addItem(pickSilk);

        // Pickaxe — Fortune
        ItemStack pickFortune = new ItemStack(Material.NETHERITE_PICKAXE);
        pickFortune.addUnsafeEnchantment(Enchantment.EFFICIENCY, 5);
        pickFortune.addUnsafeEnchantment(Enchantment.UNBREAKING, 3);
        pickFortune.addUnsafeEnchantment(Enchantment.MENDING, 1);
        pickFortune.addUnsafeEnchantment(Enchantment.FORTUNE, 3);
        setName(pickFortune, name + "'s Fortune Pickaxe");
        inv.addItem(pickFortune);

        // Axe
        ItemStack axe = new ItemStack(Material.NETHERITE_AXE);
        axe.addUnsafeEnchantment(Enchantment.EFFICIENCY, 5);
        axe.addUnsafeEnchantment(Enchantment.UNBREAKING, 3);
        axe.addUnsafeEnchantment(Enchantment.MENDING, 1);
        axe.addUnsafeEnchantment(Enchantment.SILK_TOUCH, 1);
        axe.addUnsafeEnchantment(Enchantment.SHARPNESS, 5);
        setName(axe, name + "'s Axe");
        inv.addItem(axe);

        // Shovel
        ItemStack shovel = new ItemStack(Material.NETHERITE_SHOVEL);
        shovel.addUnsafeEnchantment(Enchantment.EFFICIENCY, 5);
        shovel.addUnsafeEnchantment(Enchantment.UNBREAKING, 3);
        shovel.addUnsafeEnchantment(Enchantment.MENDING, 1);
        shovel.addUnsafeEnchantment(Enchantment.SILK_TOUCH, 1);
        setName(shovel, name + "'s Shovel");
        inv.addItem(shovel);

        // Hoe
        ItemStack hoe = new ItemStack(Material.NETHERITE_HOE);
        hoe.addUnsafeEnchantment(Enchantment.EFFICIENCY, 5);
        hoe.addUnsafeEnchantment(Enchantment.UNBREAKING, 3);
        hoe.addUnsafeEnchantment(Enchantment.MENDING, 1);
        hoe.addUnsafeEnchantment(Enchantment.FORTUNE, 3);
        setName(hoe, name + "'s Hoe");
        inv.addItem(hoe);

        // === MISC TOOLS ===
        // Shears
        ItemStack shears = new ItemStack(Material.SHEARS);
        shears.addUnsafeEnchantment(Enchantment.UNBREAKING, 3);
        shears.addUnsafeEnchantment(Enchantment.MENDING, 1);
        shears.addUnsafeEnchantment(Enchantment.EFFICIENCY, 5);
        setName(shears, name + "'s Shears");
        inv.addItem(shears);

        // Flint and Steel
        ItemStack flintSteel = new ItemStack(Material.FLINT_AND_STEEL);
        flintSteel.addUnsafeEnchantment(Enchantment.UNBREAKING, 3);
        flintSteel.addUnsafeEnchantment(Enchantment.MENDING, 1);
        setName(flintSteel, name + "'s Flint and Steel");
        inv.addItem(flintSteel);

        // === FISHING ROD ===
        ItemStack fishingRod = new ItemStack(Material.FISHING_ROD);
        fishingRod.addUnsafeEnchantment(Enchantment.UNBREAKING, 3);
        fishingRod.addUnsafeEnchantment(Enchantment.MENDING, 1);
        fishingRod.addUnsafeEnchantment(Enchantment.LUCK_OF_THE_SEA, 3);
        fishingRod.addUnsafeEnchantment(Enchantment.LURE, 3);
        setName(fishingRod, name + "'s Fishing Rod");
        inv.addItem(fishingRod);

        // === BOW ===
        ItemStack bow = new ItemStack(Material.BOW);
        bow.addUnsafeEnchantment(Enchantment.POWER, 5);
        bow.addUnsafeEnchantment(Enchantment.UNBREAKING, 3);
        bow.addUnsafeEnchantment(Enchantment.MENDING, 1);
        bow.addUnsafeEnchantment(Enchantment.FLAME, 1);
        bow.addUnsafeEnchantment(Enchantment.PUNCH, 2);
        bow.addUnsafeEnchantment(Enchantment.INFINITY, 1);
        setName(bow, name + "'s Bow");
        inv.addItem(bow);

        // === CONSUMABLES ===
        // 128 golden carrots (2 stacks of 64)
        inv.addItem(new ItemStack(Material.GOLDEN_CARROT, 64));
        inv.addItem(new ItemStack(Material.GOLDEN_CARROT, 64));

        // 128 arrows (2 stacks of 64)
        inv.addItem(new ItemStack(Material.ARROW, 64));
        inv.addItem(new ItemStack(Material.ARROW, 64));

        // === SHULKER BOXES ===
        inv.addItem(new ItemStack(Material.SHULKER_BOX));
        inv.addItem(new ItemStack(Material.SHULKER_BOX));
        inv.addItem(new ItemStack(Material.SHULKER_BOX));
        inv.addItem(new ItemStack(Material.SHULKER_BOX));
    }

    private void setName(ItemStack item, String name) {
        item.editMeta(meta -> meta.displayName(Component.text(name)));
    }
}
