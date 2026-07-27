import { invoke } from "@tauri-apps/api/core";
import * as THREE from "three";
import "./styles.css";

type ModelAssignment = {
  provider: string;
  model: string;
  endpoint: string | null;
};

type TeamLeader = {
  id: string;
  display_name: string;
  professional_role: string;
  department: string;
  model: ModelAssignment;
};

type CrewManifest = {
  leaders: TeamLeader[];
};

type WarpCoreStatus = {
  queued: number;
  transmitting: number;
  retry: number;
  acknowledged: number;
  rejected: number;
  dirty_documents: number;
};

const canvas = document.querySelector<HTMLCanvasElement>("#viewport");
if (!canvas) throw new Error("3D viewport is missing");

const renderer = new THREE.WebGLRenderer({
  canvas,
  antialias: true,
  powerPreference: "high-performance",
});
renderer.setPixelRatio(Math.min(window.devicePixelRatio, 2));
renderer.setSize(window.innerWidth, window.innerHeight);
renderer.outputColorSpace = THREE.SRGBColorSpace;

const scene = new THREE.Scene();
scene.background = new THREE.Color(0x03050a);
scene.fog = new THREE.FogExp2(0x03050a, 0.025);

const camera = new THREE.PerspectiveCamera(
  48,
  window.innerWidth / window.innerHeight,
  0.1,
  200,
);
camera.position.set(0, 5.5, 18);

scene.add(new THREE.HemisphereLight(0x9aa8ff, 0x100812, 1.8));
const keyLight = new THREE.PointLight(0xffa95c, 90, 34);
keyLight.position.set(0, 8, 3);
scene.add(keyLight);

const room = new THREE.Group();
scene.add(room);

const floor = new THREE.Mesh(
  new THREE.CircleGeometry(13, 64),
  new THREE.MeshStandardMaterial({
    color: 0x111725,
    metalness: 0.55,
    roughness: 0.45,
  }),
);
floor.rotation.x = -Math.PI / 2;
room.add(floor);

const table = new THREE.Mesh(
  new THREE.CapsuleGeometry(3.6, 4.5, 10, 28),
  new THREE.MeshStandardMaterial({
    color: 0x2c3344,
    metalness: 0.75,
    roughness: 0.28,
  }),
);
table.scale.set(1.4, 0.3, 0.72);
table.rotation.z = Math.PI / 2;
table.position.y = 1.25;
room.add(table);

const tableLight = new THREE.Mesh(
  new THREE.TorusGeometry(3.2, 0.055, 12, 64),
  new THREE.MeshBasicMaterial({ color: 0xf6be4b }),
);
tableLight.rotation.x = Math.PI / 2;
tableLight.scale.y = 0.62;
tableLight.position.y = 1.75;
room.add(tableLight);

const stationColours = [0xf58b7a, 0xf6be4b, 0xc39ee7, 0x6f97dd, 0xdc4e4e, 0x69d09b];
const stations: THREE.Mesh[] = [];

for (let index = 0; index < 6; index += 1) {
  const angle = (index / 6) * Math.PI * 2;
  const station = new THREE.Mesh(
    new THREE.CylinderGeometry(0.5, 0.68, 1.2, 20),
    new THREE.MeshStandardMaterial({
      color: 0x171d2b,
      emissive: stationColours[index],
      emissiveIntensity: 0.18,
      metalness: 0.4,
      roughness: 0.35,
    }),
  );
  station.position.set(Math.sin(angle) * 6.4, 0.65, Math.cos(angle) * 4.8);
  station.lookAt(0, 0.8, 0);
  station.userData.baseIntensity = 0.18;
  stations.push(station);
  room.add(station);
}

const starGeometry = new THREE.BufferGeometry();
const starPositions = new Float32Array(1600 * 3);
for (let index = 0; index < starPositions.length; index += 1) {
  starPositions[index] = THREE.MathUtils.randFloatSpread(120);
}
starGeometry.setAttribute("position", new THREE.BufferAttribute(starPositions, 3));
scene.add(
  new THREE.Points(
    starGeometry,
    new THREE.PointsMaterial({ color: 0x9eb5de, size: 0.08, transparent: true, opacity: 0.72 }),
  ),
);

const bridgePosition = new THREE.Vector3(0, 5.5, 18);
const briefingPosition = new THREE.Vector3(9.5, 4.2, 9.5);
const targetPosition = new THREE.Vector3().copy(bridgePosition);
let highlightedStation = -1;

function setScene(name: "bridge" | "briefing") {
  const isBriefing = name === "briefing";
  targetPosition.copy(isBriefing ? briefingPosition : bridgePosition);
  document.querySelector("#audit-panel")?.classList.toggle("hidden", !isBriefing);
  setText("#scene-code", isBriefing ? "BRIEFING / 02" : "BRIDGE / 01");
  setText("#scene-title", isBriefing ? "Audit Review" : "Senior Staff");
  setText(
    "#scene-description",
    isBriefing
      ? "Walk through evidence, dissent, risk, and remediation."
      : "Independent models. One accountable command structure.",
  );
  document.querySelectorAll<HTMLButtonElement>(".nav[data-scene]").forEach((button) => {
    button.classList.toggle("active", button.dataset.scene === name);
  });
}

document.querySelectorAll<HTMLButtonElement>("[data-scene]").forEach((button) => {
  button.addEventListener("click", () => {
    setScene(button.dataset.scene as "bridge" | "briefing");
  });
});

document.querySelector("#next-finding")?.addEventListener("click", () => {
  highlightedStation = (highlightedStation + 1) % stations.length;
  targetPosition.set(
    stations[highlightedStation].position.x * 1.25,
    3.1,
    stations[highlightedStation].position.z * 1.25 + 4.5,
  );
});

function setText(selector: string, text: string) {
  const element = document.querySelector(selector);
  if (element) element.textContent = text;
}

async function connectWarpCore() {
  try {
    const [crew, status] = await Promise.all([
      invoke<CrewManifest>("get_crew_manifest"),
      invoke<WarpCoreStatus>("get_warp_core_status"),
    ]);
    setText(
      "#system-status",
      `WARP CORE ONLINE · ${crew.leaders.length} OFFICERS · ${status.queued + status.retry} TO BASE`,
    );
  } catch (error) {
    setText("#system-status", `WARP CORE OFFLINE · ${String(error)}`);
  }
}

const clock = new THREE.Clock();
function animate() {
  const elapsed = clock.getElapsedTime();
  camera.position.lerp(targetPosition, 0.035);
  camera.lookAt(0, 1.1, 0);
  tableLight.rotation.z = elapsed * 0.035;

  stations.forEach((station, index) => {
    const material = station.material as THREE.MeshStandardMaterial;
    const active = index === highlightedStation;
    material.emissiveIntensity = THREE.MathUtils.lerp(
      material.emissiveIntensity,
      active ? 1.8 + Math.sin(elapsed * 4) * 0.25 : station.userData.baseIntensity,
      0.08,
    );
  });

  renderer.render(scene, camera);
  requestAnimationFrame(animate);
}

window.addEventListener("resize", () => {
  camera.aspect = window.innerWidth / window.innerHeight;
  camera.updateProjectionMatrix();
  renderer.setSize(window.innerWidth, window.innerHeight);
});

void connectWarpCore();
animate();
