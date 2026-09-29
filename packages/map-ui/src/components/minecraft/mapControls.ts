import L from "leaflet";

/** Squaremap projects one block to 1 / 2^maxZoom CRS units; one block is one metre. */
export function mapScale(maxZoom: number, zoom: number, viewportWidth: number) {
  const metresPerPixel = 2 ** (maxZoom - zoom), width = Math.min(500, viewportWidth * 0.65);
  const limit = width * metresPerPixel;
  if (!Number.isFinite(limit) || limit <= 0) return null;
  const magnitude = 10 ** Math.floor(Math.log10(limit)), fraction = limit / magnitude;
  const metres = (fraction >= 8 ? 8 : fraction >= 5 ? 5 : fraction >= 4 ? 4 : fraction >= 2 ? 2 : 1) * magnitude;
  const unit = metres >= 1000 ? "km" : "m";
  const divisor = unit === "km" ? 1000 : 1;
  const format = (value: number) => (value / divisor).toLocaleString("en", { maximumFractionDigits: 9, useGrouping: false });
  return { metres, pixels: metres / metresPerPixel, label: `${format(metres)} ${unit}`, ticks: [0, 1, 2, 3, 4].map(index => format(metres * index / 4)) };
}

/** Controls only change the camera; Home does not select or inspect a world location. */
export function createMapControls(map: L.Map, maxZoom: number, clearInspection: () => void) {
  const home = new L.Control({ position: "topleft" }), scale = new L.Control({ position: "bottomleft" });
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
  const line = L.DomUtil.create("div", "minecraft-map-scale-line", scaleContainer);
  for (let index = 0; index <= 4; index += 1) {
    const mark = L.DomUtil.create("span", "minecraft-map-scale-mark", line);
    mark.style.left = `${index * 25}%`;
  }
  const labels = L.DomUtil.create("div", "minecraft-map-scale-labels", scaleContainer);
  const tickLabels = [0, 1, 2, 3, 4].map(index => {
    const tick = L.DomUtil.create("span", "minecraft-map-scale-label", labels);
    tick.style.left = `${index * 25}%`;
    return tick;
  });
  scaleContainer.setAttribute("role", "img");
  L.DomEvent.disableClickPropagation(scaleContainer); L.DomEvent.disableScrollPropagation(scaleContainer);
  const update = () => {
    const current = mapScale(maxZoom, map.getZoom(), map.getSize().x);
    scaleContainer.hidden = current === null;
    if (!current) return;
    line.style.width = `${current.pixels}px`; labels.style.width = `${current.pixels}px`;
    for (const [index, tick] of tickLabels.entries()) tick.textContent = index === 4 ? current.label : current.ticks[index] ?? "";
    scaleContainer.setAttribute("aria-label", `Map scale: ${current.label}`);
    scaleContainer.dataset.metres = String(current.metres); scaleContainer.dataset.pixels = String(current.pixels);
  };
  scale.onAdd = () => { map.on("zoomend resize", update); update(); return scaleContainer; };
  scale.onRemove = () => { map.off("zoomend resize", update); };
  home.addTo(map); scale.addTo(map);
  return () => { home.remove(); scale.remove(); };
}
