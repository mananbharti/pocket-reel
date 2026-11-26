import * as THREE from "three";
import { RoundedBoxGeometry } from "three/examples/jsm/geometries/RoundedBoxGeometry.js";
import { RoomEnvironment } from "three/examples/jsm/environments/RoomEnvironment.js";

export interface SceneController {
  setProgress(p: number): void;
  setPointer(x: number, y: number): void;
  setVisible(v: boolean): void;
}

interface Pose {
  x: number;
  y: number;
  z: number;
  rx: number;
  ry: number;
  rz: number;
  pix: number;
}

const lerp = (a: number, b: number, t: number) => a + (b - a) * t;
const smooth = (t: number) => t * t * (3 - 2 * t);

function poseAt(p: number, mobile: boolean): Pose {
  // Keyframes along the hero + story scroll.
  const desk: Pose[] = [
    { x: 1.85, y: -0.05, z: 0, rx: 0.14, ry: -0.55, rz: 0.05, pix: 0 },
    { x: 1.95, y: 0, z: 0.2, rx: 0.06, ry: -0.22, rz: 0.02, pix: 0 },
    { x: 1.95, y: 0, z: 0.75, rx: 0.02, ry: -0.05, rz: 0, pix: 0.45 },
    { x: 1.9, y: 0.02, z: 1.05, rx: -0.02, ry: 0.1, rz: -0.01, pix: 1 },
    { x: 1.9, y: 0.6, z: 0.2, rx: -0.25, ry: 0.6, rz: -0.06, pix: 1 },
  ];
  const mob: Pose[] = desk.map((k, i) => ({ ...k, x: 0, y: 1.05 + (i === 4 ? 0.5 : 0), z: k.z - 0.9, ry: k.ry * 0.6 }));
  const keys = mobile ? mob : desk;
  const f = Math.min(keys.length - 1.0001, Math.max(0, p * (keys.length - 1)));
  const i = Math.floor(f);
  const t = smooth(f - i);
  const a = keys[i];
  const b = keys[i + 1];
  return {
    x: lerp(a.x, b.x, t),
    y: lerp(a.y, b.y, t),
    z: lerp(a.z, b.z, t),
    rx: lerp(a.rx, b.rx, t),
    ry: lerp(a.ry, b.ry, t),
    rz: lerp(a.rz, b.rz, t),
    pix: lerp(a.pix, b.pix, t),
  };
}

function shadowTexture(): THREE.Texture {
  const c = document.createElement("canvas");
  c.width = c.height = 256;
  const g = c.getContext("2d")!;
  const grad = g.createRadialGradient(128, 128, 0, 128, 128, 128);
  grad.addColorStop(0, "rgba(0,0,0,0.55)");
  grad.addColorStop(0.45, "rgba(0,0,0,0.22)");
  grad.addColorStop(1, "rgba(0,0,0,0)");
  g.fillStyle = grad;
  g.fillRect(0, 0, 256, 256);
  const t = new THREE.CanvasTexture(c);
  t.colorSpace = THREE.SRGBColorSpace;
  return t;
}

export function startScene(
  canvas: HTMLCanvasElement,
  opts: { videoSrc: string; reducedMotion: boolean },
): SceneController {
  const renderer = new THREE.WebGLRenderer({ canvas, antialias: true, alpha: true, powerPreference: "high-performance" });
  renderer.setPixelRatio(Math.min(window.devicePixelRatio, 2));
  renderer.outputColorSpace = THREE.SRGBColorSpace;
  renderer.toneMapping = THREE.ACESFilmicToneMapping;
  renderer.toneMappingExposure = 1.05;

  const scene = new THREE.Scene();
  const camera = new THREE.PerspectiveCamera(28, 1, 0.1, 100);
  camera.position.set(0, 0, 10);

  const pmrem = new THREE.PMREMGenerator(renderer);
  scene.environment = pmrem.fromScene(new RoomEnvironment(), 0.04).texture;

  const key = new THREE.DirectionalLight(0xffffff, 1.4);
  key.position.set(3, 5, 6);
  scene.add(key);
  scene.add(new THREE.AmbientLight(0xffffff, 0.25));

  const darkQuery = window.matchMedia("(prefers-color-scheme: dark)");
  const bodyMat = new THREE.MeshPhysicalMaterial({
    metalness: 0.9,
    roughness: 0.34,
    clearcoat: 0.5,
    clearcoatRoughness: 0.3,
  });
  const wheelMat = new THREE.MeshPhysicalMaterial({ metalness: 0.2, roughness: 0.55, clearcoat: 0.3 });
  const buttonMat = new THREE.MeshPhysicalMaterial({ metalness: 0.85, roughness: 0.3 });
  const applyTheme = () => {
    const dark = darkQuery.matches;
    bodyMat.color.set(dark ? 0x34486e : 0x4a6aa8);
    wheelMat.color.set(dark ? 0x1b2233 : 0x22304d);
    buttonMat.color.set(dark ? 0x5c6f96 : 0x8fa5cf);
  };
  applyTheme();
  darkQuery.addEventListener?.("change", applyTheme);

  const device = new THREE.Group();
  scene.add(device);

  const W = 1.72;
  const H = 2.86;
  const D = 0.34;
  device.add(new THREE.Mesh(new RoundedBoxGeometry(W, H, D, 12, 0.22), bodyMat));

  // Glass front over the screen area.
  const glass = new THREE.Mesh(
    new RoundedBoxGeometry(1.5, 1.2, 0.02, 6, 0.008),
    new THREE.MeshPhysicalMaterial({ color: 0x060607, roughness: 0.06, metalness: 0, clearcoat: 1, clearcoatRoughness: 0.05 }),
  );
  glass.position.set(0, 0.62, D / 2 + 0.002);
  device.add(glass);

  // Screen: the video, pixelated by a uniform as the story progresses.
  const video = document.createElement("video");
  video.src = opts.videoSrc;
  video.muted = true;
  video.loop = true;
  video.playsInline = true;
  video.crossOrigin = "anonymous";
  video.setAttribute("aria-hidden", "true");
  video.play().catch(() => {});
  const vtex = new THREE.VideoTexture(video);
  vtex.colorSpace = THREE.SRGBColorSpace;
  vtex.minFilter = THREE.LinearFilter;
  vtex.magFilter = THREE.LinearFilter;

  const pixUniform = { value: 0 };
  const screenMat = new THREE.MeshBasicMaterial({ map: vtex, toneMapped: false });
  screenMat.onBeforeCompile = (shader) => {
    shader.uniforms.uPix = pixUniform;
    shader.fragmentShader = shader.fragmentShader
      .replace("#include <common>", "#include <common>\nuniform float uPix;")
      .replace(
        "#include <map_fragment>",
        `
        // Fine (640 px) to classic-player (160 x 120) pixels.
        vec2 res = mix(vec2(640.0, 480.0), vec2(160.0, 120.0), uPix);
        vec2 puv = (floor(vMapUv * res) + 0.5) / res;
        vec2 uv = mix(vMapUv, puv, smoothstep(0.0, 0.25, uPix));
        vec4 sampledDiffuseColor = texture2D(map, uv);
        #ifdef DECODE_VIDEO_TEXTURE
          sampledDiffuseColor = sRGBTransferEOTF(sampledDiffuseColor);
        #endif
        vec2 cell = fract(vMapUv * res);
        float grid = smoothstep(0.0, 0.12, cell.x) * smoothstep(0.0, 0.12, cell.y);
        sampledDiffuseColor.rgb *= mix(1.0, 0.8 + 0.2 * grid, smoothstep(0.5, 1.0, uPix));
        vec2 v = vMapUv - 0.5;
        sampledDiffuseColor.rgb *= 1.0 - dot(v, v) * 0.35;
        diffuseColor *= sampledDiffuseColor;
        `,
      );
  };
  const screen = new THREE.Mesh(new THREE.PlaneGeometry(1.36, 1.02), screenMat);
  screen.position.set(0, 0.62, D / 2 + 0.013);
  device.add(screen);

  // Glass sheen over the screen.
  const sheen = new THREE.Mesh(
    new THREE.PlaneGeometry(1.5, 1.2),
    new THREE.MeshBasicMaterial({ color: 0xffffff, transparent: true, opacity: 0.05, depthWrite: false }),
  );
  sheen.position.set(0, 0.62, D / 2 + 0.016);
  device.add(sheen);

  // Four-way pad, centre button and two side keys (a generic player layout).
  const pad = new THREE.Mesh(new RoundedBoxGeometry(0.84, 0.84, 0.05, 6, 0.2), wheelMat);
  pad.position.set(0, -0.74, D / 2);
  device.add(pad);
  const centre = new THREE.Mesh(new THREE.CylinderGeometry(0.15, 0.15, 0.04, 64), buttonMat);
  centre.rotation.x = Math.PI / 2;
  centre.position.set(0, -0.74, D / 2 + 0.03);
  device.add(centre);
  const markMat = new THREE.MeshBasicMaterial({ color: 0x9aa3b5 });
  const tri = new THREE.Shape();
  tri.moveTo(0, 0.05);
  tri.lineTo(0.045, -0.025);
  tri.lineTo(-0.045, -0.025);
  tri.closePath();
  for (const [x, y, r] of [
    [0, -0.45, 0],
    [0, -1.03, Math.PI],
    [-0.29, -0.74, Math.PI / 2],
    [0.29, -0.74, -Math.PI / 2],
  ] as const) {
    const m = new THREE.Mesh(new THREE.ShapeGeometry(tri), markMat);
    m.position.set(x, y, D / 2 + 0.027);
    m.rotation.z = r;
    device.add(m);
  }
  for (const x of [-0.63, 0.63]) {
    const key = new THREE.Mesh(new THREE.CylinderGeometry(0.085, 0.085, 0.04, 48), buttonMat);
    key.rotation.x = Math.PI / 2;
    key.position.set(x, -1.12, D / 2 + 0.005);
    device.add(key);
  }

  // Hold switch and headphone jack on the top edge.
  const hold = new THREE.Mesh(new RoundedBoxGeometry(0.28, 0.06, 0.12, 4, 0.02), buttonMat);
  hold.position.set(-0.45, H / 2 + 0.01, 0);
  device.add(hold);
  const jack = new THREE.Mesh(new THREE.CylinderGeometry(0.05, 0.05, 0.02, 32), new THREE.MeshBasicMaterial({ color: 0x0a0a0a }));
  jack.position.set(0.45, H / 2 + 0.002, 0);
  device.add(jack);

  const shadow = new THREE.Mesh(
    new THREE.PlaneGeometry(3.2, 1.1),
    new THREE.MeshBasicMaterial({ map: shadowTexture(), transparent: true, depthWrite: false, opacity: 0.55 }),
  );
  shadow.rotation.x = -Math.PI / 2;
  scene.add(shadow);

  let progress = 0;
  let pointer = { x: 0, y: 0 };
  let visible = true;
  let mobile = false;
  const cur: Pose = { ...poseAt(0, false) };

  function resize() {
    const w = window.innerWidth;
    const h = window.innerHeight;
    renderer.setSize(w, h, false);
    camera.aspect = w / h;
    mobile = w < 860;
    camera.fov = mobile ? 38 : 28;
    camera.updateProjectionMatrix();
  }
  resize();
  window.addEventListener("resize", resize);

  const clock = new THREE.Clock();
  let raf = 0;
  function frame() {
    raf = 0;
    const dt = Math.min(0.05, clock.getDelta());
    const t = clock.elapsedTime;
    const target = poseAt(progress, mobile);
    const k = opts.reducedMotion ? 1 : 1 - Math.exp(-dt * 5.5);
    for (const key of Object.keys(cur) as (keyof Pose)[]) cur[key] = lerp(cur[key], target[key], k);

    const float = opts.reducedMotion ? 0 : Math.sin(t * 0.9) * 0.05;
    const sway = opts.reducedMotion ? 0 : Math.sin(t * 0.6) * 0.015;
    const px = opts.reducedMotion ? 0 : pointer.x;
    const py = opts.reducedMotion ? 0 : pointer.y;
    device.position.set(cur.x, cur.y + float, cur.z);
    device.rotation.set(cur.rx + py * 0.08, cur.ry + px * 0.12, cur.rz + sway);
    pixUniform.value = cur.pix;
    shadow.position.set(cur.x, cur.y - H / 2 - 0.45, cur.z);
    (shadow.material as THREE.MeshBasicMaterial).opacity = 0.5 - float * 2;

    renderer.render(scene, camera);
    if (visible && !document.hidden) raf = requestAnimationFrame(frame);
  }
  const kick = () => {
    if (!raf && visible) {
      clock.getDelta();
      raf = requestAnimationFrame(frame);
    }
  };
  document.addEventListener("visibilitychange", kick);
  kick();

  return {
    setProgress(p) {
      progress = Math.min(1, Math.max(0, p));
      kick();
    },
    setPointer(x, y) {
      pointer = { x, y };
      kick();
    },
    setVisible(v) {
      visible = v;
      if (v) {
        video.play().catch(() => {});
        kick();
      } else video.pause();
    },
  };
}
