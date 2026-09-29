import AdminWorkspace from "../components/admin/AdminWorkspace";
import MinecraftControls from "../components/minecraft/MinecraftControls";
import { t } from "../state/i18n";
import "../styles/minecraft-controls.css";

export default function AdminMinecraft() {
  return (
    <AdminWorkspace>
      <div class="minecraft-admin">
        <header>
          <h1>{t("minecraft.admin_title")}</h1>
          <a href="/minecraft">{t("top_bar.nav.minecraft")}</a>
        </header>
        <MinecraftControls />
      </div>
    </AdminWorkspace>
  );
}
