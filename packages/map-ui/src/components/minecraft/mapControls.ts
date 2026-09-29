import L from "leaflet";

/** Squaremap projects one block to 1 / 2^maxZoom CRS units; one block is one metre. */
export function mapScale(maxZoom: number, zoom: number, viewportWidth: number) {
  const metresPerPixel = 2 ** (maxZoom - zoom), width = Math.min(120, viewportWidth / 4);
  const limit = width * metresPerPixel;
  if (!Number.isFinite(limit) || limit <= 0) return null;
  const magnitude = 10 ** Math.floor(Math.log10(limit)), fraction = limit / magnitude;
  const metres = (fraction >= 5 ? 5 : fraction >= 2 ? 2 : 1) * magnitude;
  return { metres, pixels: metres / metresPerPixel, label: `${(metres / 1000).toLocaleString("en", { maximumFractionDigits: 12, useGrouping: false })} km` };
}

/** Controls only change the camera; Home does not select or inspect a world location. */
export function createMapControls(map: L.Map, maxZoom: number, clearInspection: () => void) {
  const home = new L.Control({ position: "topleft" }), scale = new L.Control({ position: "topleft" });
  const homeContainer = L.DomUtil.create("div", "leaflet-bar minecraft-home-control");
  const button = document.createElement("button");
  button.type = "button"; button.className = "minecraft-map-home"; button.title = "Home (X 0, Z 0)";
  button.setAttribute("aria-label", "Home"); button.textContent = "⌂"; homeContainer.append(button);
  const returnHome = () => { clearInspection(); map.panTo([0, 0], { animate: false }); };
  button.addEventListener("click", returnHome);
  L.DomEvent.disableClickPropagation(homeContainer); L.DomEvent.disableScrollPropagation(homeContainer);
  home.onAdd = () => homeContainer;
  home.onRemove = () => { button.removeEventListener("click", returnHome); };

  const scaleContainer = L.DomUtil.create("div", "minecraft-map-scale");
  const label = L.DomUtil.create("span", "minecraft-map-scale-label", scaleContainer);
  const line = L.DomUtil.create("div", "minecraft-map-scale-line", scaleContainer);
  scaleContainer.setAttribute("role", "img");
  L.DomEvent.disableClickPropagation(scaleContainer); L.DomEvent.disableScrollPropagation(scaleContainer);
  const update = () => {
    const current = mapScale(maxZoom, map.getZoom(), map.getSize().x);
    scaleContainer.hidden = current === null;
    if (!current) return;
    line.style.width = `${current.pixels}px`; label.textContent = current.label;
    scaleContainer.setAttribute("aria-label", `Map scale: ${current.label}`);
    scaleContainer.dataset.metres = String(current.metres); scaleContainer.dataset.pixels = String(current.pixels);
  };
  scale.onAdd = () => { map.on("zoomend resize", update); update(); return scaleContainer; };
  scale.onRemove = () => { map.off("zoomend resize", update); };
  home.addTo(map); scale.addTo(map);
  return () => { home.remove(); scale.remove(); };
}
