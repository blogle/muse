import * as THREE from 'three';
import { OrbitControls } from 'three/addons/controls/OrbitControls.js';
import './style.css';
import wave2Snapshot from './fixtures/wave2-generic.json';

type Field = {
  Scalar?: number[];
  Vector?: number[][];
  Bool?: boolean[];
  Category?: number[];
  Index?: number[];
};
type Snapshot = {
  mesh: { positions: number[][]; triangles: number[][] };
  fields: Record<string, Field>;
  networks: Record<string, { edges: number[][]; values?: number[] | null }>;
  step: number;
};

const root = document.querySelector<HTMLDivElement>('#viewer')!;
const scalarSelect = document.querySelector<HTMLSelectElement>('#scalar-select')!;
const paletteSelect = document.querySelector<HTMLSelectElement>('#palette-select')!;
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

function fieldKind(field: Field | undefined): keyof Field | undefined {
  if (!field) return undefined;
  return (['Scalar', 'Bool', 'Category', 'Index', 'Vector'] as const).find((kind) => field[kind] !== undefined);
}

function colorForCategory(value: number, categories: number[]) {
  const index = [...new Set(categories)].sort((a, b) => a - b).indexOf(value);
  return new THREE.Color().setHSL((0.04 + (index * 0.61803398875)) % 1, 0.68, 0.54);
}

function divergingColor(value: number, limit: number) {
  const t = Math.max(-1, Math.min(1, value / limit));
  const color = new THREE.Color();
  if (t < 0) color.lerpColors(new THREE.Color('#315ea8'), new THREE.Color('#f4f1df'), t + 1);
  else color.lerpColors(new THREE.Color('#f4f1df'), new THREE.Color('#b83b4b'), t);
  return color;
}

function setScalar() {
  const name = scalarSelect.value;
  const field = snapshot.fields[name];
  const kind = fieldKind(field);
  const geometry = surface.geometry;
  const vertexColors: number[] = [];
  if (!field || !kind || kind === 'Vector') {
    for (let i = 0; i < snapshot.mesh.positions.length; i++) vertexColors.push(0.24, 0.48, 0.68);
    range.textContent = 'No display field selected';
  } else {
    const values = field[kind]!;
    const numericValues = kind === 'Bool' ? (values as boolean[]).map((value) => value ? 1 : 0) : values as number[];
    const min = Math.min(...numericValues);
    const max = Math.max(...numericValues);
    const limit = Math.max(Math.abs(min), Math.abs(max), 1);
    const mode = paletteSelect.value === 'auto'
      ? kind === 'Bool' ? 'bool' : kind === 'Category' || kind === 'Index' ? 'categorical' : min < 0 && max > 0 ? 'diverging' : 'sequential'
      : paletteSelect.value;
    numericValues.forEach((value) => {
      const color = mode === 'bool'
        ? value ? new THREE.Color('#f1c75b') : new THREE.Color('#263746')
        : mode === 'categorical'
          ? colorForCategory(value, numericValues)
          : mode === 'diverging'
            ? divergingColor(value, limit)
            : new THREE.Color().setHSL(0.66 - 0.62 * ((value - min) / (max - min || 1)), 0.78, 0.48);
      vertexColors.push(color.r, color.g, color.b);
    });
    range.textContent = `${name} · ${kind} · ${min.toPrecision(4)} — ${max.toPrecision(4)}`;
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
  const field = snapshot.fields[scalarSelect.value];
  const kind = fieldKind(field);
  const values = kind && kind !== 'Vector' ? field?.[kind] : undefined;
  const cellValue = values ? [a, b, c].map((index) => values[index]).join(', ') : 'n/a';
  hover.textContent = `Cell ${hit.faceIndex} · ${scalarSelect.value}: ${cellValue}`;
}

async function load() {
  const paths: { path: string; label: string; data?: Snapshot }[] = [
    { path: 'snapshots/canonical-small.json', label: 'Step 0' },
    { path: 'snapshots/canonical-small-step1.json', label: 'Step 1' },
    { path: 'snapshots/wave1-generated.json', label: 'Generated debug' },
    { path: '', label: 'Wave 2 local fixture', data: wave2Snapshot as Snapshot },
  ];
  const snapshots = await Promise.all(paths.map(async (path) => {
    if (path.data) return path.data;
    const response = await fetch(`/${path.path}`);
    if (!response.ok) throw new Error(`Unable to load ${path.path}: ${response.status}`);
    return await response.json() as Snapshot;
  }));
  stepSelect.replaceChildren(...paths.map((item, index) => new Option(item.label, String(index))));
  const chooseSnapshot = () => {
    snapshot = snapshots[Number(stepSelect.value)];
    if (surface) scene.remove(surface);
    const geometry = new THREE.BufferGeometry();
    geometry.setAttribute('position', new THREE.Float32BufferAttribute(snapshot.mesh.positions.flat(), 3));
    geometry.setIndex(snapshot.mesh.triangles.flat());
    geometry.computeVertexNormals();
    surface = new THREE.Mesh(geometry, new THREE.MeshStandardMaterial({ vertexColors: true, roughness: 0.78, side: THREE.DoubleSide }));
    scene.add(surface);
    options(scalarSelect, Object.keys(snapshot.fields).filter((name) => fieldKind(snapshot.fields[name]) !== 'Vector'));
    options(vectorSelect, Object.keys(snapshot.fields).filter((name) => snapshot.fields[name].Vector), 'None');
    options(networkSelect, Object.keys(snapshot.networks), 'None');
    setScalar(); setVectors(); setNetwork();
  };
  stepSelect.addEventListener('change', chooseSnapshot);
  chooseSnapshot();
  scalarSelect.addEventListener('change', setScalar);
  paletteSelect.addEventListener('change', setScalar);
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
