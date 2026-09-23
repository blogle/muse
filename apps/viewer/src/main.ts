import * as THREE from 'three';
import { OrbitControls } from 'three/addons/controls/OrbitControls.js';
import './style.css';

type Snapshot = {
  mesh: { positions: number[][]; triangles: number[][] };
  fields: Record<string, { Scalar?: number[]; Vector?: number[][] }>;
  networks: Record<string, { edges: number[][]; values?: number[] | null }>;
  step: number;
};

const root = document.querySelector<HTMLDivElement>('#viewer')!;
const scalarSelect = document.querySelector<HTMLSelectElement>('#scalar-select')!;
const vectorSelect = document.querySelector<HTMLSelectElement>('#vector-select')!;
const networkSelect = document.querySelector<HTMLSelectElement>('#network-select')!;
const stepSelect = document.querySelector<HTMLSelectElement>('#step-select')!;
const range = document.querySelector<HTMLDivElement>('#range')!;
const hover = document.querySelector<HTMLDivElement>('#hover')!;
const scene = new THREE.Scene();
scene.background = new THREE.Color('#101821');
const camera = new THREE.PerspectiveCamera(38, 1, 0.1, 100);
const renderer = new THREE.WebGLRenderer({ antialias: true, preserveDrawingBuffer: true });
renderer.setPixelRatio(Math.min(devicePixelRatio, 2));
root.append(renderer.domElement);
const controls = new OrbitControls(camera, renderer.domElement);
controls.enableDamping = true;
scene.add(new THREE.HemisphereLight(0xddeeff, 0x28313b, 2.2));
const keyLight = new THREE.DirectionalLight(0xffffff, 2.2);
keyLight.position.set(3, 4, 5);
scene.add(keyLight);

let snapshot: Snapshot;
let surface: THREE.Mesh;
let vectorGroup = new THREE.Group();
let networkGroup = new THREE.Group();
const raycaster = new THREE.Raycaster();
const pointer = new THREE.Vector2();

function options(select: HTMLSelectElement, values: string[], empty?: string) {
  select.replaceChildren();
  if (empty) select.add(new Option(empty, ''));
  for (const value of values) select.add(new Option(value, value));
}

function setScalar() {
  const name = scalarSelect.value;
  const values = snapshot.fields[name]?.Scalar;
  const geometry = surface.geometry;
  const vertexColors: number[] = [];
  if (!values) {
    for (let i = 0; i < snapshot.mesh.positions.length; i++) vertexColors.push(0.24, 0.48, 0.68);
    range.textContent = 'No scalar field selected';
  } else {
    const min = Math.min(...values);
    const max = Math.max(...values);
    const span = max - min || 1;
    values.forEach((value) => {
      const t = (value - min) / span;
      const color = new THREE.Color().setHSL(0.66 - 0.62 * t, 0.78, 0.48);
      vertexColors.push(color.r, color.g, color.b);
    });
    range.textContent = `${name}: ${min.toPrecision(4)} — ${max.toPrecision(4)}`;
  }
  geometry.setAttribute('color', new THREE.Float32BufferAttribute(vertexColors, 3));
  hover.textContent = 'Hover a cell to inspect it';
}

function setVectors() {
  scene.remove(vectorGroup);
  vectorGroup = new THREE.Group();
  const vectors = snapshot.fields[vectorSelect.value]?.Vector;
  if (vectors) vectors.forEach((raw, i) => {
    const origin = new THREE.Vector3(...snapshot.mesh.positions[i]).multiplyScalar(1.012);
    const normal = origin.clone().normalize();
    const vector = new THREE.Vector3(...raw);
    const direction = vector.addScaledVector(normal, -vector.dot(normal));
    if (direction.lengthSq() > 1e-8) {
      direction.normalize();
      vectorGroup.add(new THREE.ArrowHelper(direction, origin, 0.22, 0xfff1a6, 0.08, 0.055));
    }
  });
  scene.add(vectorGroup);
}

function setNetwork() {
  scene.remove(networkGroup);
  networkGroup = new THREE.Group();
  const networkName = networkSelect.value;
  const network = networkName ? snapshot.networks[networkName] : undefined;
  if (network) {
    const points: THREE.Vector3[] = [];
    for (const [a, b] of network.edges) {
      points.push(new THREE.Vector3(...snapshot.mesh.positions[a]).multiplyScalar(1.025));
      points.push(new THREE.Vector3(...snapshot.mesh.positions[b]).multiplyScalar(1.025));
    }
    const geometry = new THREE.BufferGeometry().setFromPoints(points);
    networkGroup.add(new THREE.LineSegments(geometry, new THREE.LineBasicMaterial({ color: 0xff5b70 })));
  }
  scene.add(networkGroup);
}

function resize() {
  const width = root.clientWidth;
  const height = root.clientHeight;
  camera.aspect = width / height;
  camera.updateProjectionMatrix();
  renderer.setSize(width, height);
}

function resetCamera() {
  camera.position.set(2.6, 2.1, 2.6);
  controls.target.set(0, 0, 0);
  controls.update();
}

function animate() {
  requestAnimationFrame(animate);
  controls.update();
  renderer.render(scene, camera);
}

function inspect(event: PointerEvent) {
  const rect = renderer.domElement.getBoundingClientRect();
  pointer.set(((event.clientX - rect.left) / rect.width) * 2 - 1, -((event.clientY - rect.top) / rect.height) * 2 + 1);
  raycaster.setFromCamera(pointer, camera);
  const hit = raycaster.intersectObject(surface)[0];
  if (!hit || hit.faceIndex == null) {
    hover.textContent = 'Hover a cell to inspect it';
    return;
  }
  const [a, b, c] = snapshot.mesh.triangles[hit.faceIndex];
  const values = snapshot.fields[scalarSelect.value]?.Scalar;
  const cellValue = values ? (values[a] + values[b] + values[c]) / 3 : Number.NaN;
  hover.textContent = `Cell ${hit.faceIndex} · ${scalarSelect.value}: ${Number.isFinite(cellValue) ? cellValue.toPrecision(5) : 'n/a'}`;
}

async function load() {
  const paths = ['snapshots/canonical-small.json', 'snapshots/canonical-small-step1.json'];
  const snapshots = await Promise.all(paths.map(async (path) => {
    const response = await fetch(`/${path}`);
    if (!response.ok) throw new Error(`Unable to load ${path}: ${response.status}`);
    return await response.json() as Snapshot;
  }));
  options(stepSelect, snapshots.map((item) => String(item.step)));
  const chooseSnapshot = () => {
    snapshot = snapshots[stepSelect.selectedIndex];
    if (surface) scene.remove(surface);
    const geometry = new THREE.BufferGeometry();
    geometry.setAttribute('position', new THREE.Float32BufferAttribute(snapshot.mesh.positions.flat(), 3));
    geometry.setIndex(snapshot.mesh.triangles.flat());
    geometry.computeVertexNormals();
    surface = new THREE.Mesh(geometry, new THREE.MeshStandardMaterial({ vertexColors: true, roughness: 0.78, side: THREE.DoubleSide }));
    scene.add(surface);
    options(scalarSelect, Object.keys(snapshot.fields).filter((name) => snapshot.fields[name].Scalar));
    options(vectorSelect, Object.keys(snapshot.fields).filter((name) => snapshot.fields[name].Vector), 'None');
    options(networkSelect, Object.keys(snapshot.networks), 'None');
    setScalar(); setVectors(); setNetwork();
  };
  stepSelect.addEventListener('change', chooseSnapshot);
  chooseSnapshot();
  scalarSelect.addEventListener('change', setScalar);
  vectorSelect.addEventListener('change', setVectors);
  networkSelect.addEventListener('change', setNetwork);
  document.querySelector('#reset-camera')!.addEventListener('click', resetCamera);
  renderer.domElement.addEventListener('pointermove', inspect);
  new ResizeObserver(resize).observe(root);
  resetCamera(); resize(); animate();
}

load().catch((error: unknown) => {
  console.error('Unable to load snapshot', error);
  hover.textContent = 'Unable to load snapshot';
});
